use crate::Sink;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::{Duration, Instant};

use hyper::body::Bytes;
use tokio_stream::StreamExt;

const RADIO_TIMEOUT: Duration = Duration::from_secs(60 * 5);
const RADIO_SAMPLERATE: u32 = 48000;
const RADIO_KBITRATE: i32 = 300;
const RADIO_QUALITY: u8 = 5;
const RADIO_PRELOAD: usize = 128 * 1024;

struct ServerState {
    index: Arc<crate::RadioIndex>,
    running: Mutex<
        weak_table::WeakValueHashMap<String, Weak<tokio::sync::broadcast::Sender<(Bytes, Bytes)>>>,
    >,
    metadata: RwLock<HashMap<String, String>>,
}

impl ServerState {
    fn new(index: crate::RadioIndex) -> Self {
        let mut metadata = HashMap::new();
        for station in index.keys() {
            metadata.insert(
                station.clone(),
                index.get_name(station).ok().as_deref().unwrap_or("Sprunk").to_owned(),
            );
        }

        Self {
            index: Arc::new(index),
            running: Mutex::new(weak_table::WeakValueHashMap::new()),
            metadata: RwLock::new(metadata),
        }
    }

    fn can_handle(&self, req: &hyper::Request<hyper::Body>) -> bool {
        if req.method() != hyper::Method::GET {
            return false;
        }
        let mut path = req.uri().path();
        if let Some(idx) = path.rfind("/") {
            path = &path[idx + 1..];
        }
        if !self.index.contains_key(path) {
            return false;
        }
        return true;
    }

    fn serve(
        self: &Arc<Self>,
        req: hyper::Request<hyper::Body>,
    ) -> anyhow::Result<hyper::Response<hyper::Body>> {
        let mut path = req.uri().path();
        if let Some(idx) = path.rfind("/") {
            path = &path[idx + 1..];
        }
        let path = path.to_owned();

        let rx = {
            let mut running = self
                .running
                .lock()
                .map_err(|_| anyhow::anyhow!("could not create station"))?;
            if let Some(tx) = running.get(&path) {
                tx.subscribe()
            } else {
                let index = self.index.clone();
                let (tx, rx) = tokio::sync::broadcast::channel(32);
                let tx = Arc::new(tx);
                let metadata = self
                    .metadata
                    .read()
                    .map_err(|_| anyhow::anyhow!("could not read metadata"))?;
                let reset_metadata = metadata
                    .get(&path)
                    .ok_or_else(|| anyhow::anyhow!("could not read station metadata"))?;
                let output = ServerOutputStream {
                    sender: tx.clone(),
                    state: self.clone(),
                    path: path.clone(),
                    reset_metadata: reset_metadata.clone(),
                    timeout: None,
                    chunks: VecDeque::new(),
                };
                running.insert(path.clone(), tx);
                // this must be an honest-to-god thread, because it never yields
                // this could be fixed in the future, but for now...
                let state = self.clone();
                std::thread::spawn(move || {
                    if let Ok(enc) = crate::encoder::Mp3::new(
                        RADIO_SAMPLERATE,
                        Some(RADIO_KBITRATE),
                        Some(RADIO_QUALITY),
                    ) {
                        let sink = crate::sink::Stream::new(output, enc).realtime();
                        let _ = index.play(path.clone(), Some(Box::new(sink)), true, move |m| {
                            if let Ok(mut metadata) = state.metadata.write() {
                                if let Some(v) = metadata.get_mut(&path) {
                                    println!("{}", m);
                                    *v = m;
                                }
                            }
                        });
                    }
                });
                rx
            }
        };
        let mut counter: usize = 0;
        let body = tokio_stream::wrappers::BroadcastStream::new(rx)
            .take_while(|r| r.is_ok())
            .map(move |r| {
                let r = r.map_err(|_| anyhow::anyhow!("end of stream"));
                counter += 1;
                if counter == 1 {
                    r.map(|t| t.0)
                } else {
                    r.map(|t| t.1)
                }
            });
        let mut response = hyper::Response::new(hyper::Body::wrap_stream(body));
        response
            .headers_mut()
            .insert(hyper::header::CONTENT_TYPE, "audio/mpeg".parse()?);
        Ok(response)
    }

    fn status_json(&self, icecast: bool) -> anyhow::Result<hyper::Response<hyper::Body>> {
        // mimic status-json.xsl if icecast is true
        let mut body = String::new();
        if icecast {
            body += "{\"icestats\": {\"source\": ";
        }
        body += "[";
        let mut first = true;
        let metadata = self
            .metadata
            .read()
            .map_err(|_| anyhow::anyhow!("could not read metadata"))?;
        for (station, title) in metadata.iter() {
            // FIXME json escaping
            if !first {
                body += ", ";
            }
            first = false;
            body += "{\"listenurl\": \"./";
            body += station;
            body += "\", \"title\": \"";
            body += title;
            body += "\"}";
        }
        body += "]";
        if icecast {
            body += "}}";
        }

        let mut response = hyper::Response::new(hyper::Body::from(body));
        response
            .headers_mut()
            .insert(hyper::header::CONTENT_TYPE, "application/json".parse()?);
        Ok(response)
    }
}

struct ServerOutputStream {
    sender: Arc<tokio::sync::broadcast::Sender<(Bytes, Bytes)>>,
    state: Arc<ServerState>,
    path: String,
    reset_metadata: String,
    timeout: Option<Instant>,
    chunks: VecDeque<Bytes>,
}

impl std::io::Write for ServerOutputStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(timeout) = self.timeout {
            if Instant::now() > timeout {
                if let Ok(mut metadata) = self.state.metadata.write() {
                    metadata.insert(self.path.clone(), self.reset_metadata.clone());
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "radio timed out",
                ));
            }
        }

        let chunk = Bytes::copy_from_slice(buf);
        self.chunks.push_back(chunk.clone());
        while self.chunks.iter().map(|c| c.len()).sum::<usize>() > RADIO_PRELOAD {
            self.chunks.pop_front();
        }
        let mut preload = Vec::with_capacity(self.chunks.iter().map(|c| c.len()).sum());
        for c in self.chunks.iter() {
            preload.extend_from_slice(&c);
        }

        if let Err(_) = self.sender.send((preload.into(), chunk)) {
            if let None = self.timeout {
                self.timeout = Some(Instant::now() + RADIO_TIMEOUT);
            }
        } else {
            self.timeout = None;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub async fn server_run<P>(
    addr: &std::net::SocketAddr,
    index: crate::RadioIndex,
    staticfiles: P,
) -> anyhow::Result<()>
where
    P: AsRef<std::path::Path>,
{
    let state = Arc::new(ServerState::new(index));
    let mut static_ = hyper_staticfile::Static::new(staticfiles.as_ref());
    static_.cache_headers(Some(60 * 60 * 24));
    let service = hyper::service::make_service_fn(move |_conn| {
        let state = state.clone();
        let static_ = static_.clone();
        async move {
            Ok::<_, anyhow::Error>(hyper::service::service_fn(move |req| {
                let state = state.clone();
                let static_ = static_.clone();
                async move {
                    if state.can_handle(&req) {
                        state.serve(req)
                    } else if req.method() == &hyper::Method::GET
                        && req.uri().path().ends_with("/status-json.xsl")
                    {
                        state.status_json(true)
                    } else if req.method() == &hyper::Method::GET
                        && req.uri().path().ends_with("/status.json")
                    {
                        state.status_json(false)
                    } else {
                        static_.serve(req).await.map_err(|e| e.into())
                    }
                }
            }))
        }
    });

    let server = hyper::Server::try_bind(addr)?.serve(service);

    Ok(server.await?)
}

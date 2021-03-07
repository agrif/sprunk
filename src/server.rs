use crate::Sink;

use std::sync::{Arc, Mutex, Weak};

use tokio_stream::StreamExt;

struct ServerState {
    index: Arc<crate::RadioIndex>,
    running: Mutex<
        weak_table::WeakValueHashMap<
            String,
            Weak<tokio::sync::broadcast::Sender<hyper::body::Bytes>>,
        >,
    >,
    json: hyper::body::Bytes,
    json_icecast: hyper::body::Bytes,
}

impl ServerState {
    fn new(index: crate::RadioIndex) -> Self {
        let mut state = Self {
            index: Arc::new(index),
            running: Mutex::new(weak_table::WeakValueHashMap::new()),
            json: hyper::body::Bytes::new(),
            json_icecast: hyper::body::Bytes::new(),
        };
        state.json = state.status_json_generate(false);
        state.json_icecast = state.status_json_generate(true);
        state
    }

    fn can_handle(&self, req: &hyper::Request<hyper::Body>) -> bool {
        if req.method() != hyper::Method::GET {
            return false;
        }
        let mut path = req.uri().path();
        if path.len() >= 1 {
            path = &path[1..];
        }
        if !self.index.contains_key(path) {
            return false;
        }
        return true;
    }

    fn serve(
        &self,
        req: hyper::Request<hyper::Body>,
    ) -> anyhow::Result<hyper::Response<hyper::Body>> {
        let mut path = req.uri().path();
        if path.len() >= 1 {
            path = &path[1..];
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
                let output = ServerOutputStream { sender: tx.clone() };
                running.insert(path.clone(), tx);
                // this must be an honest-to-god thread, because it never yields
                // this could be fixed in the future, but for now...
                std::thread::spawn(move || {
                    if let Ok(enc) = crate::encoder::Mp3::new(48000, None, None) {
                        let sink = crate::sink::Stream::new(output, enc).realtime();
                        let _ = index.play(path, Some(Box::new(sink)));
                    }
                });
                rx
            }
        };
        let body = tokio_stream::wrappers::BroadcastStream::new(rx)
            .take_while(|r| r.is_ok())
            .map(|r| r.map_err(|_| anyhow::anyhow!("end of stream")));
        let mut response = hyper::Response::new(hyper::Body::wrap_stream(body));
        response
            .headers_mut()
            .insert(hyper::header::CONTENT_TYPE, "audio/mpeg".parse()?);
        Ok(response)
    }

    fn status_json_generate(&self, icecast: bool) -> hyper::body::Bytes {
        // mimic status-json.xsl if icecast is true
        let mut body = String::new();
        if icecast {
            body += "{\"icestats\": {\"source\": ";
        }
        body += "[";
        let mut first = true;
        for station in self.index.keys() {
            if let Ok(defs) = self.index.load(station) {
                // FIXME json escaping
                if !first {
                    body += ", ";
                }
                first = false;
                body += "{\"listenurl\": \"./";
                body += station;
                body += "\", \"title\": \"";
                body += defs.name.as_deref().unwrap_or("Sprunk");
                body += "\"}";
            }
        }
        body += "]";
        if icecast {
            body += "}}";
        }
        hyper::body::Bytes::from(body)
    }
    fn status_json(&self, icecast: bool) -> anyhow::Result<hyper::Response<hyper::Body>> {
        let mut response = hyper::Response::new(hyper::Body::from(if icecast {
            self.json_icecast.clone()
        } else {
            self.json.clone()
        }));
        response
            .headers_mut()
            .insert(hyper::header::CONTENT_TYPE, "application/json".parse()?);
        Ok(response)
    }
}

struct ServerOutputStream {
    sender: Arc<tokio::sync::broadcast::Sender<hyper::body::Bytes>>,
}

impl std::io::Write for ServerOutputStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .send(hyper::body::Bytes::copy_from_slice(buf))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e))?;
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

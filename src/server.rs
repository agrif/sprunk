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
}

impl ServerState {
    fn serve(
        &self,
        req: hyper::Request<hyper::Body>,
    ) -> anyhow::Result<hyper::Response<hyper::Body>> {
        if req.method() != hyper::Method::GET {
            anyhow::bail!("bad request");
        }
        let mut path = req.uri().path();
        if path.len() >= 1 {
            path = &path[1..];
        }
        if !self.index.exists(path) {
            anyhow::bail!("station does not exist");
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

pub async fn server_run(
    addr: &std::net::SocketAddr,
    index: crate::RadioIndex,
) -> anyhow::Result<()> {
    let state = Arc::new(ServerState {
        index: Arc::new(index),
        running: Mutex::new(weak_table::WeakValueHashMap::new()),
    });
    let service = hyper::service::make_service_fn(move |_conn| {
        let state = state.clone();
        async move {
            Ok::<_, anyhow::Error>(hyper::service::service_fn(move |req| {
                let response = state.serve(req).or_else(|_| {
                    let mut not_found = hyper::Response::default();
                    *not_found.status_mut() = hyper::StatusCode::NOT_FOUND;
                    Ok::<_, anyhow::Error>(not_found)
                });
                async { response }
            }))
        }
    });

    let server = hyper::Server::try_bind(addr)?.serve(service);

    Ok(server.await?)
}

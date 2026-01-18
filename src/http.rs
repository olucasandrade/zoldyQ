use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use zoldyq::{QueueManager, ServerStats};

pub async fn run_http_server(
    host: &str,
    port: u16,
    queue_manager: Arc<QueueManager>,
    stats: Arc<ServerStats>,
    shutdown_tx: broadcast::Sender<()>,
) -> std::io::Result<()> {
    let addr: SocketAddr = format!("{}:{}", host, port).parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    
    let listener = TcpListener::bind(addr).await?;
    let mut shutdown_rx = shutdown_tx.subscribe();

    tracing::info!("HTTP health server listening on {}", addr);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _) = result?;
                let io = TokioIo::new(stream);
                let qm = queue_manager.clone();
                let st = stats.clone();

                tokio::spawn(async move {
                    let service = service_fn(move |req| {
                        handle_request(req, qm.clone(), st.clone())
                    });

                    if let Err(e) = http1::Builder::new()
                        .serve_connection(io, service)
                        .await
                    {
                        tracing::debug!("HTTP connection error: {}", e);
                    }
                });
            }
            _ = shutdown_rx.recv() => {
                tracing::info!("HTTP server shutting down");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    queue_manager: Arc<QueueManager>,
    stats: Arc<ServerStats>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let response = match req.uri().path() {
        "/health" => {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(r#"{"status":"ok"}"#)))
                .unwrap()
        }
        "/ready" => {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(r#"{"status":"ready"}"#)))
                .unwrap()
        }
        "/metrics" => {
            let connected = stats.connected_clients.load(Ordering::SeqCst);
            let total_conn = stats.total_connections.load(Ordering::SeqCst);
            let total_cmds = stats.total_commands.load(Ordering::SeqCst);
            let queue_count = queue_manager.queue_count();

            let body = format!(
                r#"{{"connected_clients":{},"total_connections":{},"total_commands":{},"queue_count":{}}}"#,
                connected, total_conn, total_cmds, queue_count
            );

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not Found")))
                .unwrap()
        }
    };

    Ok(response)
}

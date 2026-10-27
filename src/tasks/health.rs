use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub const DEFAULT_HEALTH_PORT: u16 = 8080;

pub fn start_health_server(port: u16, is_ready: Arc<AtomicBool>, start_time: Instant) {
    tokio::spawn(async move {
        let addr = format!("0.0.0.0:{}", port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => {
                tracing::info!("Health check server listening on http://{}", addr);
                l
            }
            Err(e) => {
                tracing::error!("Failed to bind health check server on {}: {}", addr, e);
                return;
            }
        };

        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::debug!("Health check accept error: {}", e);
                    continue;
                }
            };

            let is_ready = is_ready.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                match socket.read(&mut buf).await {
                    Ok(n) if n > 0 => {
                        let ready = is_ready.load(Ordering::Relaxed);
                        let uptime = start_time.elapsed().as_secs();

                        let (status_line, status_str) = if ready {
                            ("HTTP/1.1 200 OK", "healthy")
                        } else {
                            ("HTTP/1.1 503 Service Unavailable", "starting")
                        };

                        let body = format!(
                            "{{\"status\":\"{}\",\"uptime_secs\":{}}}\n",
                            status_str, uptime
                        );

                        let response = format!(
                            "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );

                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.flush().await;
                    }
                    _ => {}
                }
            });
        }
    });
}

pub async fn run_healthcheck_client(port: u16) -> bool {
    let connect_fut = async {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
        let request = b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        stream.write_all(request).await.ok()?;
        stream.flush().await.ok()?;

        let mut buf = [0u8; 256];
        let n = stream.read(&mut buf).await.ok()?;
        let response = String::from_utf8_lossy(&buf[..n]);
        Some(response.contains("200 OK"))
    };

    tokio::time::timeout(Duration::from_secs(3), connect_fut)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_healthcheck_starting_and_ready() {
        let is_ready = Arc::new(AtomicBool::new(false));
        let start_time = Instant::now();

        // Bind to ephemeral port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        start_health_server(port, is_ready.clone(), start_time);
        tokio::time::sleep(Duration::from_millis(50)).await;

        // When not ready, client should fail
        let healthy = run_healthcheck_client(port).await;
        assert!(!healthy);

        // Mark ready
        is_ready.store(true, Ordering::SeqCst);

        // When ready, client should succeed
        let healthy = run_healthcheck_client(port).await;
        assert!(healthy);
    }
}

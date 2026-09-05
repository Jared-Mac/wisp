use anyhow::{Context, bail};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, client_async_tls_with_config,
    tungstenite::handshake::client::{Request, Response},
};

/// Share the media transport's dual-stack fallback with server event streams.
/// Connecting a resolved IP must never replace the hostname used by TLS.
pub(crate) async fn connect_events(
    request: Request,
) -> anyhow::Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response)> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let host = request
            .uri()
            .host()
            .context("server events URL has no host")?;
        let port = match (request.uri().port_u16(), request.uri().scheme_str()) {
            (Some(port), _) => port,
            (None, Some("wss")) => 443,
            (None, Some("ws")) => 80,
            _ => bail!("invalid server events URL scheme"),
        };
        let socket = livekit_net::connect_tcp(host, port)
            .await
            .context("connect server events TCP")?;
        client_async_tls_with_config(request, socket, None, None)
            .await
            .context("connect server events TLS/WebSocket")
    })
    .await
    .context("server events connection timed out")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    #[tokio::test]
    async fn control_websocket_uses_resolved_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            tokio_tungstenite::accept_async(socket).await.unwrap()
        });
        let request = format!("ws://localhost:{port}/events")
            .into_client_request()
            .unwrap();
        let (_, response) = connect_events(request).await.unwrap();
        assert_eq!(response.status().as_u16(), 101);
        server.await.unwrap();
    }

    #[tokio::test]
    #[ignore = "read-only public signaling endpoint probe; no authentication or room join"]
    async fn public_signaling_endpoint_is_reachable_without_joining() {
        let url = std::env::var("WISP_TEST_SIGNALING_URL").unwrap();
        let start = std::time::Instant::now();
        let error = livekit_net::ws_client()
            .unwrap()
            .connect(url, vec![], 5_000)
            .await
            .err()
            .expect("unauthenticated probe must not join");
        assert!(
            matches!(error, livekit_net::TransportError::Http { status: 401 }),
            "expected authorization challenge, got a transport failure"
        );
        eprintln!(
            "Signaling TLS/WebSocket reached authorization in {:?}; no room joined",
            start.elapsed()
        );
    }
}

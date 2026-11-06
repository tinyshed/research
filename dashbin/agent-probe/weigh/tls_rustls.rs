
type Dialled = tokio_tungstenite::tungstenite::Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::handshake::client::Response,
)>;

async fn dial(url: &str) -> Dialled {
    tokio_tungstenite::connect_async_tls_with_config(url, None, false, connector()).await
}

/// rustls with the variant's provider, trusting the variant's roots.
fn connector() -> Option<tokio_tungstenite::Connector> {
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(provider()))
        .with_safe_default_protocol_versions()
        .ok()?
        .with_root_certificates(roots())
        .with_no_client_auth();
    Some(tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(config)))
}


type Dialled = tokio_tungstenite::tungstenite::Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::handshake::client::Response,
)>;

async fn dial(url: &str) -> Dialled {
    tokio_tungstenite::connect_async(url).await
}

//! A measurement harness, never committed to dashbin: the agent's side of its socket, a client that dials the URL
//! given, sends a greeting and reads its echo. Which TLS it links is the variant's: `dial()` comes from one of the
//! `tls_*.rs` files, and with rustls its `roots()` and `provider()` from apply.py.

use futures_util::{SinkExt as _, StreamExt as _};
use tokio_tungstenite::tungstenite::Message;

pub async fn agent(url: &str) -> bool {
    let Ok((mut socket, _)) = dial(url).await else { return false };
    if socket.send(Message::text("agent.hello")).await.is_err() {
        return false;
    }
    matches!(socket.next().await, Some(Ok(Message::Text(text))) if text.as_str() == "agent.hello")
}

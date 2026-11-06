//! A measurement harness, never committed to dashbin: the hub's side of an agent's socket, an axum route that
//! upgrades to a WebSocket and answers each message with itself, served on the address given.

use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::routing::get;

pub async fn hub(address: &str) -> bool {
    let route = get(|upgrade: WebSocketUpgrade| async move { upgrade.on_upgrade(echo) });
    let router = axum::Router::new().route("/api/v1/agent", route);
    let Ok(listener) = tokio::net::TcpListener::bind(address).await else { return false };
    axum::serve(listener, router).await.is_ok()
}

async fn echo(mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.recv().await {
        if socket.send(message).await.is_err() {
            return;
        }
    }
}

// axum and nothing else: the floor every other number sits on
#[path = "common.rs"]
mod common;

use axum::{Router, routing::get};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| async { "ok" })).route("/rss", get(|| async { common::rss() }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening");
    axum::serve(listener, app).await.unwrap();
}

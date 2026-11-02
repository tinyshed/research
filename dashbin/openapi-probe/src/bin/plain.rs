// the three routes the hub's first slice serves, with no schema: what the other two add is measured against this

use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Setup {
    code: String,
    login: String,
}

#[derive(Serialize)]
struct Token {
    token: String,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Placement {
    Hub,
    Agent { agent: String, id: String },
}

#[derive(Deserialize)]
struct NewInstance {
    id: String,
    plugin: String,
    placement: Placement,
}

#[derive(Deserialize)]
struct Run {
    on: String,
    sql: String,
}

#[derive(Serialize)]
struct FrameJson {
    columns: Vec<ColumnJson>,
    truncated: bool,
}

#[derive(Serialize)]
struct ColumnJson {
    name: String,
    values: Vec<serde_json::Value>,
    /// A decimal as its exact digits.
    total: String,
}

async fn setup(Json(setup): Json<Setup>) -> Json<Token> {
    Json(Token { token: format!("dbp_{}_{}", setup.code, setup.login) })
}

async fn instances(Json(instance): Json<NewInstance>) -> StatusCode {
    let kept = matches!(instance.placement, Placement::Hub) && !instance.id.is_empty() && !instance.plugin.is_empty();
    if kept { StatusCode::CREATED } else { StatusCode::UNPROCESSABLE_ENTITY }
}

async fn run(Json(run): Json<Run>) -> Json<FrameJson> {
    let column = ColumnJson { name: run.on, values: vec![serde_json::json!(run.sql)], total: "18.4031".to_owned() };
    Json(FrameJson { columns: vec![column], truncated: false })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let router = Router::new()
        .route("/api/v1/setup", post(setup))
        .route("/api/v1/instances", post(instances))
        .route("/api/v1/run", post(run));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7399").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

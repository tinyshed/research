// the same three routes with aide: a derive on each type, and the routes read from the handlers' own types

use std::sync::Arc;

use aide::axum::ApiRouter;
use aide::axum::routing::post;
use aide::openapi::{Info, OpenApi};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
struct Setup {
    code: String,
    login: String,
}

#[derive(Serialize, JsonSchema)]
struct Token {
    token: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Placement {
    Hub,
    Agent { agent: String, id: String },
}

#[derive(Deserialize, JsonSchema)]
struct NewInstance {
    id: String,
    plugin: String,
    placement: Placement,
}

#[derive(Deserialize, JsonSchema)]
struct Run {
    on: String,
    sql: String,
}

#[derive(Serialize, JsonSchema)]
struct FrameJson {
    columns: Vec<ColumnJson>,
    truncated: bool,
}

#[derive(Serialize, JsonSchema)]
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
    let mut api = OpenApi { info: Info { title: "probe".to_owned(), ..Info::default() }, ..OpenApi::default() };
    let router = ApiRouter::new()
        .api_route("/api/v1/setup", post(setup))
        .api_route("/api/v1/instances", post(instances))
        .api_route("/api/v1/run", post(run))
        .finish_api(&mut api);
    if std::env::args().any(|arg| arg == "--spec") {
        println!("{}", serde_json::to_string_pretty(&api).unwrap());
        return;
    }
    let spec = Arc::new(serde_json::to_value(&api).unwrap());
    let router = router.route("/api/openapi.json", get(move || async move { Json(spec.as_ref().clone()) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7399").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

// utoipa's marks on the handlers, the schema built only by a test: what a release binary carries then

use axum::http::StatusCode;
use axum::Router;
use axum::routing::post;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

#[derive(Deserialize, ToSchema)]
struct Setup {
    code: String,
    login: String,
}

#[derive(Serialize, ToSchema)]
struct Token {
    token: String,
}

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Placement {
    Hub,
    Agent { agent: String, id: String },
}

#[derive(Deserialize, ToSchema)]
struct NewInstance {
    id: String,
    plugin: String,
    placement: Placement,
}

#[derive(Deserialize, ToSchema)]
struct Run {
    on: String,
    sql: String,
}

#[derive(Serialize, ToSchema)]
struct FrameJson {
    columns: Vec<ColumnJson>,
    truncated: bool,
}

#[derive(Serialize, ToSchema)]
struct ColumnJson {
    name: String,
    values: Vec<serde_json::Value>,
    /// A decimal as its exact digits.
    total: String,
}

#[utoipa::path(post, path = "/api/v1/setup", request_body = Setup, responses((status = 200, body = Token)))]
async fn setup(Json(setup): Json<Setup>) -> Json<Token> {
    Json(Token { token: format!("dbp_{}_{}", setup.code, setup.login) })
}

#[utoipa::path(post, path = "/api/v1/instances", request_body = NewInstance, responses((status = 201), (status = 422)))]
async fn instances(Json(instance): Json<NewInstance>) -> StatusCode {
    let kept = matches!(instance.placement, Placement::Hub) && !instance.id.is_empty() && !instance.plugin.is_empty();
    if kept { StatusCode::CREATED } else { StatusCode::UNPROCESSABLE_ENTITY }
}

#[utoipa::path(post, path = "/api/v1/run", request_body = Run, responses((status = 200, body = FrameJson)))]
async fn run(Json(run): Json<Run>) -> Json<FrameJson> {
    let column = ColumnJson { name: run.on, values: vec![serde_json::json!(run.sql)], total: "18.4031".to_owned() };
    Json(FrameJson { columns: vec![column], truncated: false })
}

#[derive(OpenApi)]
#[openapi(info(title = "probe"), paths(setup, instances, run))]
struct Api;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let router = Router::new()
        .route("/api/v1/setup", post(setup))
        .route("/api/v1/instances", post(instances))
        .route("/api/v1/run", post(run));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7399").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

#[cfg(test)]
mod tests {
    use utoipa::OpenApi as _;

    #[test]
    fn the_schema_is_written_from_the_handlers() {
        let spec = super::Api::openapi().to_pretty_json().unwrap();
        std::fs::write("out/offline.json", spec).unwrap();
    }
}

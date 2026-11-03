//! The same config, and an MCP server on stdin with a tool that reads and one that writes, as dashbin mcp would serve.

use serde_json::json;
use tinystore::config::{self, Declaration, Env};
use tinystore::mcp::{self, Tool};

fn main() {
    let declaration = Declaration::from_json(json!({ "listen": { "kind": "text", "default": "127.0.0.1:7317" } }));
    let opened = declaration.and_then(|declaration| config::load("probe", declaration).env(Env::prefix("PROBE")).open());
    if let Err(error) = opened {
        return eprintln!("{error}");
    }
    let name = json!({ "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] });
    let served = mcp::server("probe", "0.0.0")
        .instructions("Dashboards of a project.")
        .tool(Tool::reads("dashboards", "The project's dashboards.", json!({ "type": "object" }), |_| {
            Ok(json!([{ "name": "shop" }]))
        }))
        .tool(Tool::writes("dashboard_save", "Saves a dashboard.", name, |args| {
            let name = args["name"].as_str().ok_or("dashboard_save takes name")?;
            Ok(json!({ "saved": name }))
        }))
        .allow(std::env::args().nth(2).as_deref())
        .serve();
    if let Err(error) = served {
        eprintln!("{error}");
    }
}

//! TinyStore's config alone, as the command line links it, to weigh its mcp feature against.

use serde_json::json;
use tinystore::config::{self, Declaration, Env};

fn main() {
    let declaration = Declaration::from_json(json!({ "listen": { "kind": "text", "default": "127.0.0.1:7317" } }));
    let opened = declaration.and_then(|declaration| config::load("probe", declaration).env(Env::prefix("PROBE")).open());
    match opened {
        Ok(config) => println!("{}", *config.get()),
        Err(error) => eprintln!("{error}"),
    }
}

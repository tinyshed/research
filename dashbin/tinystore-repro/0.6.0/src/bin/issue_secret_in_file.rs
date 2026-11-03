//! A secret is read from a committed file as from the environment, and a missing one is told to go in the file.

use serde_json::json;
use tinystore::config::{self, Declaration, Env, File};

fn main() {
    let dir = std::env::temp_dir().join("ts-repro-secret-in-file");
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let declaration = || {
        Declaration::from_json(json!({
            "db": { "group": {
                "host": { "kind": "text", "default": "localhost" },
                "password": { "kind": "text", "secret": true, "required": true },
            } },
        }))
        .unwrap()
    };
    for file in ["[db]\nhost = \"db\"\npassword = \"s3cret\"\n", "[db]\nhost = \"db\"\n"] {
        std::fs::write("app.toml", file).unwrap();
        let opened = config::load("app", declaration()).file(File::optional("app.toml")).env(Env::prefix("APP")).open();
        match opened {
            Ok(config) => {
                for line in config.explain().settings {
                    println!("{:<12} {:<9} {}", line.path, line.value, line.from);
                }
            }
            Err(error) => println!("{error}"),
        }
        println!();
    }
}

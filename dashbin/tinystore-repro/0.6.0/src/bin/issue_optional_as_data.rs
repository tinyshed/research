//! A setting with no value is declared by a type as an Option, and refused when the same is declared as data.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tinystore::config::{self, Declaration, Env, File, Secret};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Db {
    host: String,
    password: Option<Secret<String>>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct App {
    db: Db,
}

fn main() {
    let dir = std::env::temp_dir().join("ts-repro-optional-as-data");
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_current_dir(&dir).unwrap();
    std::fs::write("app.toml", "[db]\nhost = \"db\"\n").unwrap();
    let typed = config::load("app", App::default()).file(File::optional("app.toml")).env(Env::prefix("APP")).open();
    match typed {
        Ok(config) => println!("typed, Option<Secret<String>> with none given: opened, password is_none = {}", config.get().db.password.is_none()),
        Err(error) => println!("typed: {error}"),
    }
    for password in [json!({ "kind": "text", "secret": true }), json!({ "kind": "text", "secret": true, "default": null })] {
        let declared = Declaration::from_json(json!({ "db": { "group": { "host": { "kind": "text", "default": "localhost" }, "password": password.clone() } } }));
        match declared {
            Ok(_) => println!("as data, {password}: accepted"),
            Err(error) => println!("as data, {password}: {error}"),
        }
    }
}

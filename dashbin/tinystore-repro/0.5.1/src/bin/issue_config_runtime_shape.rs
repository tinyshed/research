// a program whose settings' shape it learns at run time, from the plugins it loads: postgres declares host, port,
// database, user and a secret password. a Rust declaration is a typed value, so the shape can only be a JSON value

use serde_json::json;
use tinystore::config::{Env, File, load};

const FILE: &str = r#"
[instance.aimee]
plugin = "postgres"
[instance.aimee.settings]
hots = "127.0.0.1"
database = "aimee"
user = "dashbin_ro"
password = "s3cret"
"#;

fn main() {
    let dir = std::env::temp_dir().join("ts-repro-051-runtime-shape");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("dashbin.toml");
    std::fs::write(&path, FILE).unwrap();

    // what the program knows only once postgres has said what it takes
    let declared = json!({ "instance": {} });
    let variables = [("DASHBIN_INSTANCE_AIMEE_SETTINGS_PORT", "15432")];
    let config = load("dashbin", declared).file(File::optional(&path)).env(Env::prefix("DASHBIN").from(variables)).open();
    match config {
        Ok(config) => {
            let value = config.get();
            println!("aimee's settings: {}", value["instance"]["aimee"]["settings"]);
            println!("{}", config.explain());
        }
        Err(error) => println!("refused: {error}"),
    }
}

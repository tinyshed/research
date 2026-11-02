// dashbin's project file names its databases by an id the operator picks:
// [instance.<id>] with the plugin and its settings, a password among them.
// what tinystore::config 0.5.1 makes of that, from the file and from the environment.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tinystore::config::{Env, File, Secret, load};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Project {
    instance: BTreeMap<String, Instance>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Instance {
    plugin: String,
    settings: Postgres,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Postgres {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: Secret<String>,
}

const FILE: &str = r#"
[instance.aimee]
plugin = "postgres"
[instance.aimee.settings]
host = "127.0.0.1"
port = 15432
database = "aimee"
user = "dashbin_ro"
"#;

fn main() {
    let dir = std::env::temp_dir().join("ts-repro-051-config");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("dashbin.toml");
    std::fs::write(&path, FILE).unwrap();

    let attempts: [(&str, Vec<(&str, &str)>); 4] = [
        ("the file alone", vec![]),
        ("the password by its own variable", vec![("DASHBIN_INSTANCE_AIMEE_SETTINGS_PASSWORD", "s3cret")]),
        ("the password by its _FILE variable", vec![("DASHBIN_INSTANCE_AIMEE_SETTINGS_PASSWORD_FILE", "/run/secrets/pg")]),
        ("the whole map as JSON", vec![("DASHBIN_INSTANCE", r#"{"aimee":{"plugin":"postgres","settings":{"password":"s3cret"}}}"#)]),
    ];
    for (what, variables) in attempts {
        println!("== {what}");
        let opened = load("dashbin", Project::default())
            .file(File::optional(&path))
            .env(Env::prefix("DASHBIN").from(variables))
            .open();
        match opened {
            Ok(config) => {
                let project = config.get();
                match project.instance.get("aimee") {
                    Some(aimee) => println!(
                        "aimee: plugin {:?}, host {:?}, port {}, user {:?}, password given: {}",
                        aimee.plugin,
                        aimee.settings.host,
                        aimee.settings.port,
                        aimee.settings.user,
                        !aimee.settings.password.reveal().is_empty()
                    ),
                    None => println!("no instance aimee"),
                }
                let explained = format!("{}", config.explain());
                println!("explain says the password as {}", if explained.contains("s3cret") { "s3cret, in clear" } else { "hidden or absent" });
                println!("{explained}");
            }
            Err(error) => println!("refused: {error}"),
        }
    }
}

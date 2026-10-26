use serde::{Deserialize, Serialize};
use tinystore::config::{Env, File, Live, Secret};

// what an agent beside someone's database reads at start: where the hub is, and the database's password
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct Agent {
    hub: String,
    database: Secret<String>,
}

// the same with a setting that may change while it runs
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct LiveAgent {
    hub: String,
    burst: Live<u32>,
}

impl Default for LiveAgent {
    fn default() -> Self {
        LiveAgent { hub: String::new(), burst: Live::new(4) }
    }
}

fn listing(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn main() -> tinystore::Result<()> {
    let work = std::env::temp_dir().join("config-no-store-repro");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    // the file is the operator's, readable by the agent alone, so the password may sit in it
    let toml = "hub = \"wss://dashbin.example.com/agent\"\ndatabase = \"postgres://reader:secret@127.0.0.1/shop\"\n";
    std::fs::write(work.join("agent.toml"), toml).unwrap();

    let config = tinystore::config::load("agent", Agent::default()).file(File::optional(work.join("agent.toml"))).env(Env::prefix("AGENT_")).open()?;
    println!("hub from the file: {}", config.get().hub);
    println!("what reading two settings left beside the file: {:?}", listing(&work));

    let live = tinystore::config::load("agent", LiveAgent::default()).file(File::optional(work.join("agent.toml"))).open();
    println!("a live setting without a store: {:?}", live.map(|_| "opened").map_err(|e| e.to_string()));
    Ok(())
}

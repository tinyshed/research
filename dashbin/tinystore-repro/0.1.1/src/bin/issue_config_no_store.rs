use serde::{Deserialize, Serialize};
use tinystore::config::{Env, File, Secret};
use tinystore::{Options, Store};

// what an agent beside someone's database reads at start: where the hub is, and the database's password
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct Agent {
    hub: String,
    database: Secret<String>,
}

fn main() -> tinystore::Result<()> {
    let work = std::env::temp_dir().join("config-no-store-repro");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("agent.toml"), "hub = \"wss://dashbin.example.com/agent\"\n").unwrap();

    // there is no config without a store: to read a toml file and the environment, a store opens a directory
    let data = work.join("data");
    let store = Store::open(&data, Options::default())?;
    let config = store.config("agent", Agent::default()).file(File::optional(work.join("agent.toml"))).env(Env::prefix("AGENT_")).open()?;
    println!("hub from the file: {}", config.get().hub);
    store.close()?;

    let mut made: Vec<_> = std::fs::read_dir(&data).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    made.sort();
    println!("what reading two settings left on disk in {}: {:?}", data.file_name().unwrap().to_string_lossy(), made);
    Ok(())
}

//! A prototype of plan chapter 8 for its spike: a hub, an agent, and the scenarios that drive them.
//!
//! `agent-probe hub`, `agent-probe agent`, `agent-probe drive [scenario]`.

mod agent;
mod drive;
mod hub;
mod watched;
mod wire;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("hub") => hub::run().await,
        Some("agent") => agent::run().await,
        Some("drive") => drive::run(args.get(1).cloned()).await,
        _ => eprintln!("agent-probe hub | agent | drive [scenario]"),
    }
}

//! What crosses an agent's socket, chapter 8's slice 7 protocol in its simplest form: every message a JSON document
//! in a text message, named by its `method`, and a log line for each event a scenario reads back.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The call protocol's one version.
pub const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Message {
    #[serde(rename = "agent.hello")]
    Hello { agent: String, versions: Vec<u32>, instances: Vec<Announced> },
    #[serde(rename = "agent.welcome")]
    Welcome { version: u32, approved: Vec<String> },
    #[serde(rename = "call")]
    Call { id: u64, instance: String, function: String, args: Value, rows: usize, millis: u64 },
    #[serde(rename = "answer")]
    Answer { id: u64, frame: Value },
    #[serde(rename = "error")]
    Failed { id: u64, kind: String, message: String },
    #[serde(rename = "$cancel")]
    Cancel { id: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Announced {
    pub id: String,
    pub plugin: String,
}

pub fn encode(message: &Message) -> String {
    serde_json::to_string(message).unwrap_or_default()
}

pub fn decode(text: &str) -> Option<Message> {
    serde_json::from_str(text).ok()
}

/// Seconds since 1970, to the microsecond, as every log line stamps itself.
pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs_f64()).unwrap_or_default()
}

/// One JSON line on stdout: when, what, and what about it.
pub fn log(event: &str, fields: Value) {
    let mut line = json!({ "at": now(), "event": event });
    if let (Some(line), Value::Object(fields)) = (line.as_object_mut(), fields) {
        line.extend(fields);
    }
    println!("{line}");
}

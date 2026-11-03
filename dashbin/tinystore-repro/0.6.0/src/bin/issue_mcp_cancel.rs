//! A tool's call can't be cancelled, and every message after it waits for it, a ping too.

use std::io::Cursor;
use std::time::{Duration, Instant};

use serde_json::json;
use tinystore::mcp::{self, Tool};

fn main() {
    let started = Instant::now();
    let input = [
        json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "repro", "version": "0" } } }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "slow_read", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 1, "reason": "the person moved on" } }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
    ]
    .iter()
    .map(|message| format!("{message}\n"))
    .collect::<String>();
    let mut output = Vec::new();
    let served = mcp::server("repro", "0.0.0")
        .tool(Tool::reads("slow_read", "Reads for three seconds, as a heavy statement does.", json!({ "type": "object" }), move |_| {
            std::thread::sleep(Duration::from_secs(3));
            Ok(json!({ "read": "all of it" }))
        }))
        .serve_on(Cursor::new(input), &mut output);
    served.unwrap();
    for line in String::from_utf8(output).unwrap().lines().skip(1) {
        println!("{line}");
    }
    println!("all answered after {:.1} s", started.elapsed().as_secs_f64());
}

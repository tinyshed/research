//! What every archive case shares: the marks run.py samples memory between,
//! results, the services each case starts beside it, their HTTP, and the
//! bytes a directory holds.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

pub type Fallible<T> = Result<T, Box<dyn std::error::Error>>;

/// A line run.py reads: a phase starts.
pub fn mark(phase: &str) {
    println!("{}", json!({ "mark": phase, "at": unix_ms(SystemTime::now()) }));
    let _ = std::io::stdout().flush();
}

pub fn result(value: Value) {
    println!("{}", json!({ "result": value }));
    let _ = std::io::stdout().flush();
}

pub fn unix_ms(at: SystemTime) -> f64 {
    at.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

/// The arguments as --name value pairs.
pub struct Args(BTreeMap<String, String>);

impl Args {
    pub fn parse() -> Fallible<(String, Args)> {
        let mut list = std::env::args().skip(1);
        let command = list.next().ok_or("a command: metrics, logs or prepare")?;
        let mut out = BTreeMap::new();
        while let Some(name) = list.next() {
            let name = name.strip_prefix("--").ok_or_else(|| format!("{name} is no --name"))?.to_string();
            out.insert(name, list.next().unwrap_or_default());
        }
        Ok((command, Args(out)))
    }

    pub fn get(&self, name: &str) -> Fallible<String> {
        Ok(self.0.get(name).cloned().ok_or_else(|| format!("--{name} is needed"))?)
    }

    pub fn or(&self, name: &str, otherwise: &str) -> String {
        self.0.get(name).cloned().unwrap_or_else(|| otherwise.to_string())
    }
}

/// A service this case runs, stopped by SIGTERM when dropped.
pub struct Service {
    child: Child,
}

impl Service {
    pub fn start(name: &'static str, command: &mut Command, ready: &str, log: &Path) -> Fallible<(Service, f64)> {
        let log = fs::File::create(log)?;
        let child = command.stdout(log.try_clone()?).stderr(log).stdin(Stdio::null()).spawn()?;
        let mut service = Service { child };
        let began = Instant::now();
        loop {
            if let Some(status) = service.child.try_wait()? {
                return Err(format!("{name} exited with {status} before it was ready").into());
            }
            if get(ready).map(|(status, _)| status == 200).unwrap_or(false) {
                return Ok((service, began.elapsed().as_secs_f64()));
            }
            if began.elapsed() > Duration::from_secs(180) {
                return Err(format!("{name} was not ready within three minutes").into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Stops it as a container's stop would, SIGTERM, and waits for its exit.
    pub fn stop(mut self) -> Fallible<()> {
        terminate(&mut self.child)
    }
}

fn terminate(child: &mut Child) -> Fallible<()> {
    if child.try_wait()?.is_none() {
        Command::new("kill").arg("-TERM").arg(child.id().to_string()).status()?;
        let began = Instant::now();
        while child.try_wait()?.is_none() {
            if began.elapsed() > Duration::from_secs(120) {
                child.kill()?;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    child.wait()?;
    Ok(())
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = terminate(&mut self.child);
    }
}

/// HTTP over loopback, the body read to its end.
pub fn get(url: &str) -> Fallible<(u16, Vec<u8>)> {
    answered(ureq::get(url).timeout(Duration::from_secs(600)).call())
}

pub fn post(url: &str, content_type: &str, body: &[u8]) -> Fallible<(u16, Vec<u8>)> {
    answered(ureq::post(url).timeout(Duration::from_secs(600)).set("Content-Type", content_type).send_bytes(body))
}

fn answered(answer: Result<ureq::Response, ureq::Error>) -> Fallible<(u16, Vec<u8>)> {
    let response = match answer {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return Err(error.into()),
    };
    let status = response.status();
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body)?;
    Ok((status, body))
}

/// A successful answer's body, or an error naming the request and the answer.
pub fn ok(what: &str, answer: Fallible<(u16, Vec<u8>)>) -> Fallible<Vec<u8>> {
    let (status, body) = answer?;
    if !(200..300).contains(&status) {
        return Err(format!("{what}: {status} {}", String::from_utf8_lossy(&body[..body.len().min(500)])).into());
    }
    Ok(body)
}

/// The bytes the files under a directory hold on disk and as their sizes,
/// by the directory's first level, so that a report divides a service's
/// footprint into its index, its data and its log.
pub fn disk(path: &Path) -> Value {
    let mut parts = BTreeMap::<String, (u64, u64)>::new();
    let mut total = (0u64, 0u64);
    let mut stack: Vec<(PathBuf, String)> = vec![(path.to_path_buf(), String::new())];
    while let Some((dir, part)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            let name = if part.is_empty() { entry.file_name().to_string_lossy().into_owned() } else { part.clone() };
            if meta.is_dir() {
                stack.push((entry.path(), name));
            } else {
                let sizes = (meta.blocks() * 512, meta.len());
                let slot = parts.entry(name).or_default();
                slot.0 += sizes.0;
                slot.1 += sizes.1;
                total.0 += sizes.0;
                total.1 += sizes.1;
            }
        }
    }
    let parts: BTreeMap<String, Value> =
        parts.into_iter().map(|(name, (allocated, apparent))| (name, json!({ "allocated": allocated, "apparent": apparent }))).collect();
    json!({ "allocated": total.0, "apparent": total.1, "parts": parts })
}

/// Waits until `done` says so, at most `limit`, polling every half second.
pub fn until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let began = Instant::now();
    while began.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

/// The median of `repeat` runs of `run` after one not counted, in ms, with
/// what the last run answered.
pub fn timed(name: &str, repeat: usize, mut run: impl FnMut() -> Fallible<u64>) -> Fallible<()> {
    let mut size = run()?;
    let mut times = Vec::with_capacity(repeat);
    for _ in 0..repeat {
        let began = Instant::now();
        size = run()?;
        times.push(began.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    result(json!({
        "query": name, "repeat": repeat, "medianMs": times[times.len() / 2],
        "minMs": times[0], "maxMs": times[times.len() - 1], "size": size,
    }));
    Ok(())
}

/// A number from a Prometheus-style /metrics page, summed over its series.
pub fn scraped(page: &str, name: &str) -> Option<f64> {
    let mut found = None;
    for line in page.lines() {
        if line.starts_with(name) && matches!(line.as_bytes().get(name.len()), Some(b' ' | b'{')) {
            let value: f64 = line.rsplit(' ').next()?.parse().ok()?;
            *found.get_or_insert(0.0) += value;
        }
    }
    found
}

//! `see`, a prototype of chapter 11's `dashbin see`: a store's row of numbers
//! and screen of panels, made by lemmesee (`export.py` until the core is
//! ported), in a terminal to keep and drop panels in, remembered in
//! `.dashbin/see.toml`; and the same frame as a picture.
//!
//!     see <doc.json> [--state <dir>]
//!     see <doc.json> --shot <out.png> [--size 160x48] [--press x,down,f] [--px 16]
//!     see <doc.json> --html <out.html>
//!
//! `--press` plays keys before the picture is taken: a name (left, right,
//! up, down, enter, esc) or a character, separated by commas.

use see_prototype::{doc, html, shot, state, ui};

use std::path::PathBuf;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

fn key_of(name: &str) -> Option<KeyCode> {
    Some(match name {
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        s if s.chars().count() == 1 => KeyCode::Char(s.chars().next()?),
        _ => return None,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let Some(path) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("see <doc.json> [--state <dir>] [--shot <out.png> --size 160x48 --press x,down,f]");
        std::process::exit(2);
    };
    let fail = |what: &str, e: String| -> ! {
        eprintln!("{what}: {e}");
        std::process::exit(1);
    };
    let value: serde_json::Value = std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string())).unwrap_or_else(|e| fail(path, e));
    // a snapshot is drawn by the core here; a document, as export.py or --doc wrote it
    let value = if value.get("tables").is_some() {
        let user = opt("--user").or_else(|| value.get("role").and_then(|r| r.as_str()).map(str::to_string)).unwrap_or_else(|| "postgres".into());
        let database = value.get("database").and_then(|d| d.as_str()).unwrap_or_default().to_string();
        let run = if let Some(host) = opt("--ssh") {
            see_prototype::core::run::Runner::Ssh { host, key: opt("--key"), container: opt("--container").unwrap_or_else(|| fail("--ssh", "needs --container".into())), user, database }
        } else if let Some(container) = opt("--docker") {
            see_prototype::core::run::Runner::Docker { container, user, database }
        } else {
            fail(path, "a snapshot needs --docker <container> or --ssh <host> --container <container>".into())
        };
        let ranker = opt("--ranker").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../lemmesee/corpus/round-1l/learn/luna-and-deepseek/ranker.txt")));
        let model = see_prototype::core::model::Model::load(&ranker).unwrap_or_else(|e| fail("ranker", e));
        let dir = std::path::Path::new(path).parent().map(PathBuf::from).unwrap_or_default();
        let name = opt("--name").unwrap_or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "store".into()));
        let about = std::fs::read_to_string(dir.join("runs.json")).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|r| r.get("note").and_then(|n| n.as_str()).map(str::to_string)).unwrap_or_default();
        let interactive = opt("--shot").is_none() && opt("--html").is_none() && !args.iter().any(|a| a == "--doc-only");
        // one line while it works, as a build tool's: a spinner, the step in hand, the time
        let step = std::sync::Arc::new(std::sync::Mutex::new(String::from("reading the schema")));
        let shared = step.clone();
        let tell = move |e: see_prototype::core::build::Event| {
            use see_prototype::core::build::Event as E;
            let mut s = shared.lock().unwrap();
            match e {
                E::Step(t) => *s = t.to_string(),
                E::Done(_) => {}
                E::Ran(a, b) => *s = format!("running the panels {a}/{b}"),
            }
        };
        let colour = std::env::var_os("NO_COLOR").is_none();
        let paint = |code: &str, t: &str| if colour { format!("\x1b[{code}m{t}\x1b[0m") } else { t.to_string() };
        let started = std::time::Instant::now();
        let label = name.clone();
        let handle = std::thread::spawn(move || see_prototype::core::build::build(&value, &name, &about, &model, &run, &tell));
        const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut tick = 0usize;
        while !handle.is_finished() {
            if interactive {
                let line = format!("  {} {}{}", paint("36", SPIN[tick % SPIN.len()]), step.lock().unwrap(), paint("2", &format!(" {:.1}s", started.elapsed().as_secs_f64())));
                eprint!("\r\x1b[2K{line}");
            }
            std::thread::sleep(Duration::from_millis(80));
            tick += 1;
        }
        let built = handle.join().unwrap_or_else(|_| fail("see", "the build failed".into()));
        let done = format!("  {} {label}{}", paint("32", "✓"), paint("2", &format!(" in {:.1}s", started.elapsed().as_secs_f64())));
        if interactive { eprintln!("\r\x1b[2K{done}") } else { eprintln!("{done}") }
        if let Some(out) = opt("--doc") {
            std::fs::write(&out, serde_json::to_string_pretty(&built).unwrap()).unwrap_or_else(|e| fail(&out, e.to_string()));
        }
        built
    } else {
        value
    };
    let doc: doc::Doc = serde_json::from_value(value).unwrap_or_else(|e| fail(path, e.to_string()));
    if args.iter().any(|a| a == "--doc-only") {
        return;
    }
    let dir = opt("--state").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".dashbin"));
    let state = state::State::load(&dir, &doc.store.name);
    let mut app = ui::App::new(doc, state);

    if let Some(out) = opt("--html") {
        let page = html::page(&app.doc, &app.panels, &app.state.choices());
        match std::fs::write(&out, page) {
            Ok(()) => println!("{out}"),
            Err(e) => {
                eprintln!("{out}: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(out) = opt("--shot") {
        let (w, h) = opt("--size").and_then(|s| s.split_once('x').map(|(a, b)| (a.parse().ok(), b.parse().ok()))).map(|(a, b)| (a.unwrap_or(160), b.unwrap_or(48))).unwrap_or((160, 48));
        for k in opt("--press").unwrap_or_default().split(',').filter(|s| !s.is_empty()).filter_map(key_of) {
            app.key(k);
        }
        let mut term = Terminal::new(TestBackend::new(w, h)).expect("a test terminal");
        term.draw(|f| ui::draw(f, &mut app)).expect("a frame");
        let px: f32 = opt("--px").and_then(|s| s.parse().ok()).unwrap_or(16.0);
        match shot::write(term.backend().buffer(), &PathBuf::from(&out), px) {
            Ok(()) => println!("{out}"),
            Err(e) => {
                eprintln!("{out}: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let mut term = ratatui::init();
    while !app.done {
        if term.draw(|f| ui::draw(f, &mut app)).is_err() {
            break;
        }
        if event::poll(Duration::from_millis(250)).unwrap_or(false) {
            if let Ok(Event::Key(k)) = event::read() {
                if k.kind == KeyEventKind::Press {
                    app.key(k.code);
                }
            }
        }
    }
    ratatui::restore();
}

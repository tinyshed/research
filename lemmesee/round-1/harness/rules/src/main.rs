//! Turns a snapshot into round 1's documents and prints each configuration's
//! five.
//!
//!   propose --snapshot <snapshot.json> --out <directory>

use rules::{names, propose, Template};
use std::collections::BTreeMap;
use std::path::Path;

fn write(dir: &Path, name: &str, value: &impl serde::Serialize) {
    let json = serde_json::to_string_pretty(value).expect("a document serialises");
    let path = dir.join(name);
    if let Err(e) = std::fs::write(&path, json + "\n") {
        eprintln!("writing {}: {e}", path.display());
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (Some(input), Some(out)) = (flag("--snapshot"), flag("--out")) else {
        eprintln!("usage: propose --snapshot <snapshot.json> --out <directory>");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&input).unwrap_or_else(|e| {
        eprintln!("reading {input}: {e}");
        std::process::exit(1);
    });
    let snapshot: model::Snapshot = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        eprintln!("{input} is not a snapshot: {e}");
        std::process::exit(1);
    });
    let proposal = propose(&snapshot, &names::hash(&bytes));

    let dir = Path::new(&out);
    std::fs::create_dir_all(dir).expect("the output directory can be made");
    write(dir, "candidates.json", &proposal.candidates);
    for s in &proposal.scores {
        write(dir, &format!("scores-{}.json", s.config.name()), s);
    }
    for p in &proposal.panels {
        write(dir, &format!("panels-{}.json", p.config.name()), p);
    }

    let candidates = &proposal.candidates.candidates;
    let mut by_template: BTreeMap<Template, usize> = BTreeMap::new();
    for c in candidates {
        *by_template.entry(c.template).or_default() += 1;
    }
    let templates: Vec<String> = by_template.iter().map(|(t, n)| format!("{t:?} {n}")).collect();
    println!(
        "{} {}: {} candidates — {}",
        proposal.candidates.engine,
        proposal.candidates.database,
        candidates.len(),
        templates.join(", ")
    );
    for (scores, panels) in proposal.scores.iter().zip(&proposal.panels) {
        let mut removed: BTreeMap<&str, usize> = BTreeMap::new();
        let mut abstained: BTreeMap<&str, usize> = BTreeMap::new();
        for s in &scores.candidates {
            if let Some(r) = &s.removed_by {
                *removed.entry(r.split(':').next().unwrap_or(r)).or_default() += 1;
            }
            if let Some(a) = &s.abstained {
                *abstained.entry(a).or_default() += 1;
            }
        }
        let list = |m: &BTreeMap<&str, usize>| m.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
        println!("\n{}: removed {} | abstained {}", scores.config.name(), list(&removed), list(&abstained));
        for (i, p) in panels.panels.iter().enumerate() {
            println!("  {}. [{:>3} {:>3}] {}", i + 1, p.total, p.chosen_at, p.title);
        }
    }
}

//! Round 1b's baseline: round 1's roles on schemas whose declarations are
//! hidden, one snapshot a line in, one guess a line out.
//!
//!   guess <snapshots.jsonl> <guesses.jsonl>
//!
//! A guess is a column the rules took for a foreign key, and to which table,
//! or for a category by its name.

use rules::roles::{infer, Kind, Role};
use std::io::{BufRead, BufReader, BufWriter, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: guess <snapshots.jsonl> <guesses.jsonl>");
        std::process::exit(2);
    };
    let reader = BufReader::new(std::fs::File::open(input).expect("the snapshots can be read"));
    let mut out = BufWriter::new(std::fs::File::create(output).expect("the guesses can be written"));
    let (mut schemas, mut guesses) = (0usize, 0usize);
    for line in reader.lines() {
        let line = line.expect("a line");
        let snapshot: model::Snapshot = serde_json::from_str(&line).expect("a snapshot");
        let schema = infer(&snapshot, false);
        for (t, table) in snapshot.tables.iter().enumerate() {
            for (c, column) in table.columns.iter().enumerate() {
                let fk = schema
                    .fks
                    .iter()
                    .find(|f| f.child == t && f.child_column == c && f.guessed)
                    .map(|f| snapshot.tables[f.parent].name.clone());
                let category_named = schema.tables[t].columns[c]
                    .get(Role::Category)
                    .is_some_and(|h| h.evidence.iter().any(|e| e.kind == Kind::Named));
                if fk.is_none() && !category_named {
                    continue;
                }
                let guess = serde_json::json!({
                    "schema": snapshot.database,
                    "table": table.name,
                    "column": column.name,
                    "fk": fk,
                    "category_named": category_named,
                });
                writeln!(out, "{guess}").expect("a guess is written");
                guesses += 1;
            }
        }
        schemas += 1;
    }
    eprintln!("{schemas} schemas, {guesses} guesses");
}

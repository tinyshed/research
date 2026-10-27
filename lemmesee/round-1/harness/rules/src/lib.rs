//! Round 1's rules: from a snapshot to candidates, and from candidates to
//! each configuration's five panels, as candidates.md specifies. No I/O: a
//! snapshot comes in, documents go out, and the same input gives the same
//! output.

pub mod generate;
pub mod names;
pub mod roles;
pub mod score;

pub use generate::{Aggregate, Candidate, ColumnRef, Template};
pub use roles::{Role, TableRole, TableRoles};
pub use score::{Config, Panel, Scored};

use model::Snapshot;
use serde::Serialize;

pub const CONFIGS: [Config; 3] = [Config::O, Config::A, Config::B1];

#[derive(Serialize, Clone, Debug)]
pub struct ForeignKey {
    pub table: String,
    pub column: String,
    pub parent: String,
    pub parent_column: String,
    pub guessed: bool,
}

/// `candidates.json`: the roles tier S gives, and every candidate.
#[derive(Serialize, Debug)]
pub struct Candidates {
    pub version: u32,
    pub snapshot: String,
    pub engine: String,
    pub database: String,
    pub roles: Vec<TableRoles>,
    pub foreign_keys: Vec<ForeignKey>,
    pub candidates: Vec<Candidate>,
}

/// `scores-<config>.json`, with the roles this configuration read.
#[derive(Serialize, Debug)]
pub struct Scores {
    pub version: u32,
    pub config: Config,
    pub snapshot: String,
    pub roles: Vec<TableRoles>,
    pub candidates: Vec<Scored>,
}

/// `panels-<config>.json`: the five, in order.
#[derive(Serialize, Debug)]
pub struct Panels {
    pub version: u32,
    pub config: Config,
    pub snapshot: String,
    pub panels: Vec<Panel>,
}

pub struct Proposal {
    pub candidates: Candidates,
    pub scores: Vec<Scores>,
    pub panels: Vec<Panels>,
}

/// Everything round 1 proposes from one snapshot; `snapshot_id` names the
/// snapshot the documents came from.
pub fn propose(snapshot: &Snapshot, snapshot_id: &str) -> Proposal {
    let s = roles::infer(snapshot, false);
    let n = roles::infer(snapshot, true);
    let candidates = generate::generate(&s);

    let mut scores = Vec::new();
    let mut panels = Vec::new();
    for config in CONFIGS {
        let schema = if config.reads_n() { &n } else { &s };
        let (scored, chosen) = score::score(&candidates, schema, config);
        scores.push(Scores {
            version: 1,
            config,
            snapshot: snapshot_id.into(),
            roles: schema.tables.clone(),
            candidates: scored,
        });
        panels.push(Panels { version: 1, config, snapshot: snapshot_id.into(), panels: chosen });
    }

    let name = |t: usize, c: usize| {
        let table = &snapshot.tables[t];
        (table.name.clone(), table.columns[c].name.clone())
    };
    let foreign_keys = s
        .fks
        .iter()
        .map(|f| {
            let (table, column) = name(f.child, f.child_column);
            let (parent, parent_column) = name(f.parent, f.parent_column);
            ForeignKey { table, column, parent, parent_column, guessed: f.guessed }
        })
        .collect();
    Proposal {
        candidates: Candidates {
            version: 1,
            snapshot: snapshot_id.into(),
            engine: snapshot.engine.clone(),
            database: snapshot.database.clone(),
            roles: s.tables.clone(),
            foreign_keys,
            candidates,
        },
        scores,
        panels,
    }
}

//! Known answers on Chinook, from the snapshots round 1's first slice took
//! of it on PostgreSQL 18.6 (`fresh`) and MySQL 8.4.13 (`none`).

use model::Snapshot;
use rules::names::norm;
use rules::roles::{infer, ColumnRoles, Schema};
use rules::{propose, Aggregate, Config, Role, TableRole, Template};
use std::collections::BTreeSet;

const POSTGRES: &str = include_str!("data/chinook-postgres-fresh.json");
const MYSQL: &str = include_str!("data/chinook-mysql-none.json");

fn load(json: &str) -> Snapshot {
    serde_json::from_str(json).expect("a snapshot")
}

fn column<'a>(schema: &'a Schema, table: &str, column: &str) -> &'a ColumnRoles {
    let t = schema.snapshot.tables.iter().position(|t| norm(&t.name) == norm(table)).expect(table);
    schema.tables[t].columns.iter().find(|c| norm(&c.column) == norm(column)).expect(column)
}

fn table<'a>(schema: &'a Schema, name: &str) -> &'a rules::TableRoles {
    schema.tables.iter().find(|t| norm(&t.table) == norm(name)).expect(name)
}

fn scores(json: &str, config: Config) -> (Vec<rules::Candidate>, Vec<rules::Scored>, Vec<String>) {
    let snapshot = load(json);
    let p = propose(&snapshot, "test");
    let i = rules::CONFIGS.iter().position(|c| *c == config).unwrap();
    let panels = p.panels[i].panels.iter().map(|p| p.id.clone()).collect();
    (p.candidates.candidates, p.scores.into_iter().nth(i).unwrap().candidates, panels)
}

#[test]
fn money_stays_money_though_it_has_few_values() {
    let snapshot = load(POSTGRES);
    let n = infer(&snapshot, true);
    // 23 and 2 distinct values: few enough to look like categories
    assert_eq!(column(&n, "invoice", "total").primary(), Some(Role::Money));
    assert_eq!(column(&n, "invoice_line", "unit_price").primary(), Some(Role::Money));
    assert!(!column(&n, "invoice", "total").has(Role::Category));
}

#[test]
fn roles_of_chinook() {
    for json in [POSTGRES, MYSQL] {
        let snapshot = load(json);
        for schema in [infer(&snapshot, false), infer(&snapshot, true)] {
            assert_eq!(column(&schema, "invoice", "invoice_date").axis(), Some(Role::Event));
            assert_eq!(column(&schema, "employee", "birth_date").axis(), None);
            assert!(column(&schema, "employee", "birth_date").has(Role::Calendar));
            assert_eq!(column(&schema, "invoice", "customer_id").primary(), Some(Role::Fk));
            assert_eq!(column(&schema, "invoice_line", "quantity").primary(), Some(Role::Quantity));
            assert_eq!(column(&schema, "invoice", "billing_country").primary(), Some(Role::Category));
            assert!(table(&schema, "invoice_line").has(TableRole::Fact));
            assert!(table(&schema, "invoice").has(TableRole::Event));
            assert!(table(&schema, "playlist_track").has(TableRole::Link));
            assert!(table(&schema, "genre").has(TableRole::Lookup));
            assert_eq!(table(&schema, "genre").label.as_deref().map(norm), Some("name".into()));
        }
    }
}

#[test]
fn nothing_is_summed_through_a_join_that_repeats_its_rows() {
    for json in [POSTGRES, MYSQL] {
        let (candidates, _, _) = scores(json, Config::B1);
        for c in candidates.iter().filter(|c| c.measure.is_some() && c.join.is_some()) {
            let (m, j) = (c.measure.as_ref().unwrap(), c.join.as_ref().unwrap());
            assert!(!m.joined && m.t == j.from.t, "{} sums its parent's column", c.title);
        }
        // An invoice's total is never reached from its lines
        assert!(!candidates.iter().any(|c| {
            c.measure.as_ref().is_some_and(|m| norm(&m.column) == "total") && norm(&c.table) == "invoiceline"
        }));
    }
}

#[test]
fn nothing_is_grouped_by_a_quantity() {
    for json in [POSTGRES, MYSQL] {
        let (candidates, _, _) = scores(json, Config::A);
        assert!(!candidates.iter().any(|c| c.category.as_ref().is_some_and(|g| norm(&g.column) == "quantity")));
    }
}

#[test]
fn a_title_says_what_is_computed_never_what_it_means() {
    let (candidates, _, _) = scores(POSTGRES, Config::B1);
    for c in &candidates {
        assert!(c.guessed);
        for word in ["revenue", "sales", "users", "active"] {
            assert!(!c.title.to_lowercase().contains(word), "{}", c.title);
        }
    }
}

#[test]
fn both_engines_propose_the_same_candidates() {
    let ids = |json| scores(json, Config::A).0.into_iter().map(|c| c.id).collect::<BTreeSet<_>>();
    assert_eq!(ids(POSTGRES), ids(MYSQL));
}

#[test]
fn the_five_are_the_same_every_run() {
    for config in rules::CONFIGS {
        assert_eq!(scores(POSTGRES, config).2, scores(POSTGRES, config).2);
    }
}

#[test]
fn never_more_than_two_panels_from_one_table() {
    for config in [Config::A, Config::B1] {
        let (candidates, _, panels) = scores(POSTGRES, config);
        assert_eq!(panels.len(), 5);
        for p in &panels {
            let table = &candidates.iter().find(|c| &c.id == p).unwrap().table;
            let same = panels.iter().filter(|q| &candidates.iter().find(|c| &c.id == *q).unwrap().table == table).count();
            assert!(same <= 2, "{config:?}: {same} panels from {table}");
        }
    }
}

#[test]
fn statistics_remove_what_the_schema_could_not() {
    let by_address = |json, config| {
        let (candidates, scored, _) = scores(json, config);
        let i = candidates
            .iter()
            .position(|c| c.template == Template::T2 && c.category.as_ref().is_some_and(|g| norm(&g.column) == "billingaddress"))
            .expect("rows of invoice by billing_address");
        scored[i].removed_by.clone()
    };
    // 59 distinct addresses over 412 invoices: only PostgreSQL's statistics say so
    assert_eq!(by_address(POSTGRES, Config::A), None);
    assert_eq!(by_address(POSTGRES, Config::B1).as_deref(), Some("H3: grouped by too many values"));
    assert_eq!(by_address(MYSQL, Config::B1), None);
}

#[test]
fn o_is_the_schema_order() {
    let (candidates, scored, panels) = scores(POSTGRES, Config::O);
    let mut eligible: Vec<usize> = (0..candidates.len()).filter(|&i| scored[i].removed_by.is_none()).collect();
    eligible.sort_by_key(|&i| candidates[i].order);
    let first: Vec<String> = eligible.iter().take(5).map(|&i| candidates[i].id.clone()).collect();
    assert_eq!(panels, first);
}

#[test]
fn a_state_beside_an_address_is_not_a_status() {
    let (candidates, scored, _) = scores(POSTGRES, Config::A);
    for (c, s) in candidates.iter().zip(&scored) {
        assert_ne!(s.abstained.as_deref(), Some("needs-population"), "{}", c.title);
    }
    // ...so an invoice's total over time may be drawn
    let total = candidates
        .iter()
        .position(|c| {
            c.template == Template::T3
                && c.aggregate == Aggregate::Sum
                && c.measure.as_ref().is_some_and(|m| norm(&m.column) == "total")
        })
        .unwrap();
    assert!(scored[total].removed_by.is_none() && scored[total].abstained.is_none());
}

#[test]
fn invoices_are_the_subject_and_a_tracks_price_is_no_amount() {
    for json in [POSTGRES, MYSQL] {
        let snapshot = load(json);
        for schema in [infer(&snapshot, false), infer(&snapshot, true)] {
            assert_eq!(table(&schema, "invoice").subject, Some(0));
            assert_eq!(table(&schema, "invoice_line").detail_of.as_deref().map(norm), Some("invoice".into()));
            // invoices hold their total, so their lines are details
            assert!(!table(&schema, "invoice_line").main_fact);
            assert_eq!(table(&schema, "track").subject, None);
            assert!(column(&schema, "track", "unit_price").has(Role::Level));
            assert!(!column(&schema, "invoice_line", "unit_price").has(Role::Level));
        }
    }
    let (candidates, scored, _) = scores(POSTGRES, Config::B1);
    for (c, s) in candidates.iter().zip(&scored) {
        if c.aggregate == Aggregate::Sum && norm(&c.table) == "track" {
            assert!(s.removed_by.is_some(), "{}", c.title);
        }
    }
}

#[test]
fn the_largest_tables_need_statistics() {
    for config in rules::CONFIGS {
        let (candidates, scored, _) = scores(POSTGRES, config);
        let i = candidates.iter().position(|c| c.template == Template::T8).unwrap();
        assert_eq!(scored[i].removed_by.is_none(), config.reads_n(), "{config:?}");
    }
}

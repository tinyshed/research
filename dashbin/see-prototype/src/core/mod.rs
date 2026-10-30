//! lemmesee's core, ported from rounds 1c to 1o and held to their Python by
//! `tests/golden.rs`: the schema read, the questions made and compiled, the
//! ranker's features and score, and the choice.

pub mod build;
pub mod charts;
pub mod generate;
pub mod model;
pub mod names;
pub mod numbers;
pub mod question;
pub mod rank;
pub mod roles;
pub mod run;
pub mod schema;
pub mod sql;

// the same Secret inside a map's value, given by a toml file this time

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tinystore::config::{File, Secret, load};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Project {
    instance: BTreeMap<String, Instance>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Instance {
    password: Secret<String>,
}

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let config = load("app", Project::default()).file(File::optional(path)).open().unwrap();
    println!("{}", config.explain());
}

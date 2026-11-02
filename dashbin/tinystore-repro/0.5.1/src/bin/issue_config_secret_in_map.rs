// a Secret inside a map's value, given by a layer: does explain keep it hidden, as it does a Secret field

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tinystore::config::{Env, Secret, load};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Project {
    token: Secret<String>,
    instance: BTreeMap<String, Instance>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Instance {
    password: Secret<String>,
}

fn main() {
    let variables = [("APP_TOKEN", "top-level-s3cret"), ("APP_INSTANCE", r#"{"aimee":{"password":"in-a-map-s3cret"}}"#)];
    let config = load("app", Project::default()).env(Env::prefix("APP").from(variables)).open().unwrap();
    let project = config.get();
    println!("token given: {}", project.token.reveal() == "top-level-s3cret");
    println!("aimee's password given: {}", project.instance["aimee"].password.reveal() == "in-a-map-s3cret");
    println!("{}", config.explain());
}

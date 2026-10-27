//! Keys nobody declared: the two misses round 1b's dry run found in the
//! rules — a link table's columns, and plurals in `-ies`.

use rules::roles::infer;

fn snapshot() -> model::Snapshot {
    let column = |name: &str| {
        serde_json::json!({
            "name": name, "position": 1, "type_name": "integer", "base_type": "integer",
            "nullable": false, "can_select": true, "features": {}
        })
    };
    let table = |name: &str, columns: &[&str], pk: &[&str]| {
        serde_json::json!({
            "schema": "public", "name": name, "kind": "table", "can_select": true, "features": {},
            "columns": columns.iter().map(|c| column(c)).collect::<Vec<_>>(),
            "constraints": [{"name": format!("{name}_pkey"), "kind": "p", "columns": pk}],
            "indexes": []
        })
    };
    serde_json::from_value(serde_json::json!({
        "version": 1, "engine": "test", "server_version": "", "database": "shop", "role": "",
        "taken_at": "", "settings": {}, "workload": {}, "queries": [],
        "tables": [
            table("categories", &["id"], &["id"]),
            table("products", &["id", "category_id"], &["id"]),
            table("tags", &["id"], &["id"]),
            table("product_tags", &["product_id", "tag_id"], &["product_id", "tag_id"]),
        ]
    }))
    .expect("a snapshot")
}

#[test]
fn a_link_tables_columns_and_an_ies_plural_are_guessed() {
    let s = snapshot();
    let schema = infer(&s, false);
    let mut guessed: Vec<(String, String, String)> = schema
        .fks
        .iter()
        .filter(|f| f.guessed)
        .map(|f| {
            let child = &s.tables[f.child];
            (child.name.clone(), child.columns[f.child_column].name.clone(), s.tables[f.parent].name.clone())
        })
        .collect();
    guessed.sort();
    let expected = [
        ("product_tags", "product_id", "products"),
        ("product_tags", "tag_id", "tags"),
        ("products", "category_id", "categories"),
    ]
    .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()));
    assert_eq!(guessed, expected);
}

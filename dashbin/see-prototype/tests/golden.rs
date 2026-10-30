//! The Rust core against lemmesee's Python, snapshot by snapshot, from the
//! dumps `golden.py` writes into `out/golden`: the tables read, every question
//! made and its veto, the kept questions' titles, identities, shapes,
//! statements, points and scores, the first features, and the five.
//!
//!     cargo test --release --test golden -- --nocapture

use std::path::Path;

use serde_json::Value;

use see_prototype::core::{generate, model::Model, question::{By, identity}, rank, roles, schema, sql};

fn ident_json(i: &see_prototype::core::question::Identity) -> Value {
    serde_json::json!([
        i.base,
        i.agg,
        i.cols.iter().map(|c| vec![c.0, c.1]).collect::<Vec<_>>(),
        i.time.map(|t| vec![t.0, t.1]),
        i.by.as_ref().map(|b| match b {
            By::Entity(t) => serde_json::json!(["entity", t]),
            By::Col(t, c) => serde_json::json!(["col", t, c]),
        }),
    ])
}

fn close(a: f64, b: &Value, tol: f64) -> bool {
    match b.as_f64() {
        Some(x) => (a - x).abs() <= tol * (1.0 + x.abs()),
        None => a.is_nan(),
    }
}

#[test]
fn golden() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("out/golden");
    let see = root.join("../../lemmesee/corpus/see");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("no golden dumps in {}; run golden.py", dir.display());
        return;
    };
    let model = Model::load(&root.join("../../lemmesee/corpus/round-1l/learn/luna-and-deepseek/ranker.txt")).expect("the ranker");
    let mut files: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    files.sort();
    let (mut ok, mut bad) = (0, Vec::new());
    let (mut n_gen, mut n_kept, mut n_feat) = (0usize, 0usize, 0usize);
    for f in files {
        let g: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let name = g["snapshot"].as_str().unwrap().to_string();
        let snap: Value = serde_json::from_str(&std::fs::read_to_string(see.join(&name).join("snapshot.json")).unwrap()).unwrap();
        let s = schema::load(&snap, true);
        let mut why: Option<String> = None;
        let mut fail = |w: String| {
            if why.is_none() {
                why = Some(w);
            }
        };
        // the tables
        for (t, gt) in s.tables.iter().zip(g["tables"].as_array().unwrap()) {
            let out: Vec<Value> = t.out.iter().map(|o| serde_json::json!([o.0, o.1])).collect();
            if t.role != gt["role"] || t.timed != gt["timed"] || t.subject.map(|x| x as u64) != gt["subject"].as_u64()
                || t.person != gt["person"] || t.label.map(|x| x as u64) != gt["label"].as_u64() || Value::Array(out.clone()) != gt["out"]
            {
                fail(format!("table {}: role {} timed {} subject {:?} person {} label {:?} out {:?} against {}", t.name, t.role, t.timed, t.subject, t.person, t.label, out, gt));
            }
        }
        // every question made, in order, and its veto
        let all = generate::generate(&s);
        n_gen += all.len();
        let gen_ = g["generated"].as_array().unwrap();
        if all.len() != gen_.len() {
            fail(format!("{} questions made against {}", all.len(), gen_.len()));
        }
        for (k, (q, gq)) in all.iter().zip(gen_).enumerate() {
            let v = rank::veto(&s, q);
            if q.title(&s) != gq["title"] || q.shape != gq["shape"] || v.map(Value::from).unwrap_or(Value::Null) != gq["veto"] {
                fail(format!("question {k}: {:?} {} veto {:?} against {}", q.title(&s), q.shape, v, gq));
                break;
            }
        }
        // the kept ones
        let qs: Vec<_> = all.into_iter().filter(|q| rank::veto(&s, q).is_none()).collect();
        let ctx = rank::context(&s);
        let gqs = g["questions"].as_array().unwrap();
        let engine = s.engine.clone();
        n_kept += qs.len().min(gqs.len());
        let mut scores = Vec::new();
        for (k, (q, gq)) in qs.iter().zip(gqs).enumerate() {
            let f = rank::features(&s, &ctx, q);
            let raw = model.score(&f);
            let pts = rank::stats_points(&s, q);
            scores.push(raw + pts);
            let id = ident_json(&identity(q));
            let sql_ = sql::compile(&s, q, &engine, None);
            if id != gq["ident"] {
                fail(format!("question {k} {:?}: identity {id} against {}", q.title(&s), gq["ident"]));
            } else if sql_ != gq["sql"] {
                fail(format!("question {k} {:?}: statement\n  {sql_}\nagainst\n  {}", q.title(&s), gq["sql"]));
            } else if !close(pts, &gq["points"], 1e-9) {
                fail(format!("question {k} {:?}: points {pts} against {}", q.title(&s), gq["points"]));
            } else if !close(raw, &gq["raw"], 1e-6) {
                fail(format!("question {k} {:?}: score {raw} against {}", q.title(&s), gq["raw"]));
            }
            if let Some(gf) = g["features"].as_array().unwrap().get(k) {
                n_feat += gf.as_object().unwrap().len();
                for (key, gv) in gf.as_object().unwrap() {
                    let ours = f.get(key.as_str()).copied().unwrap_or(f64::NAN);
                    if !close(ours, gv, 1e-12) {
                        fail(format!("question {k} {:?}: feature {key} {ours} against {gv}", q.title(&s)));
                        break;
                    }
                }
            }
        }
        if qs.len() != gqs.len() {
            fail(format!("{} kept against {}", qs.len(), gqs.len()));
        }
        let five: Vec<Value> = rank::choose(&qs, &scores, 5).into_iter().map(Value::from).collect();
        if Value::Array(five.clone()) != g["five"] {
            fail(format!("five {five:?} against {}", g["five"]));
        }
        let ids: Vec<_> = qs.iter().map(identity).collect();
        let slots: Vec<_> = qs.iter().map(|q| rank::slots(&s, q)).collect();
        for (k, (sl, gq)) in slots.iter().zip(gqs).enumerate() {
            let ours: serde_json::Map<String, Value> = sl.iter().map(|(k, v)| (k.to_string(), serde_json::json!(v))).collect();
            if Value::Object(ours.clone()) != gq["slots"] {
                fail(format!("question {k}: slots {ours:?} against {}", gq["slots"]));
                break;
            }
        }
        let screen: Vec<Value> = roles::screen(&s, &qs, &ids, &scores, &slots, 5).into_iter().map(Value::from).collect();
        if Value::Array(screen.clone()) != g["screen"] {
            fail(format!("screen {screen:?} against {}", g["screen"]));
        }
        match why {
            None => ok += 1,
            Some(w) => bad.push(format!("{name}: {w}")),
        }
    }
    for b in &bad {
        eprintln!("✗ {b}\n");
    }
    eprintln!("{ok} snapshots agree, {} do not; {n_gen} questions made, {n_kept} kept compared with their statements and scores, {n_feat} features", bad.len());
    assert!(bad.is_empty(), "{} snapshots differ from Python", bad.len());
}

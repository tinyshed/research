//! The round's runner. `build` indexes a cohort in one process and `query`
//! answers its questions in another, so that what the query process holds is
//! the index and the questions, never the corpus. Every result is a JSON
//! line on stdout; each question's answer goes to `<out>/raw/`.
//!
//! ```text
//! lab build <corpus> <out> <cohort> <analysis> <fts|tantivy>
//! lab query <out> <cohort> <analysis> <fts|tantivy> <plan,…> <passes> <cut> <clean|typos> <exact|fuzzy>
//! lab tokens <analysis> <text>
//! ```

mod analyze;
mod blocks;
mod corpus;
mod fts;
mod plan;
#[cfg(feature = "tantivy")]
mod tant;
mod typo;

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::analyze::{Analysis, Analyzer};
use crate::plan::{Backend, Plan};
use crate::typo::Typos;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("build") => build(&args[1..]),
        Some("query") => query(&args[1..]),
        Some("write") => write(&args[1..]),
        Some("check") => check(&args[1..]),
        Some("tokens") => {
            let analyzer = Analyzer::new(Analysis::named(&args[1]));
            for token in analyzer.tokens(&args[2]) {
                println!("{}\t{}", token.surface, token.stem.unwrap_or_default());
            }
        }
        _ => panic!("lab build|query|write|check|tokens"),
    }
}

#[derive(Serialize, Deserialize)]
struct Asked {
    id: String,
    text: String,
    judged: HashMap<String, u8>,
}

fn index_path(out: &Path, cohort: &str, analysis: &str, backend: &str) -> PathBuf {
    out.join(format!("{cohort}-{analysis}-{backend}"))
}

fn build(args: &[String]) {
    let [corpus, out, cohort, analysis, backend] = args else {
        panic!("build <corpus> <out> <cohort> <analysis> <backend>")
    };
    let started = Instant::now();
    let loaded = corpus::load(Path::new(corpus), cohort);
    let load_seconds = started.elapsed().as_secs_f64();
    let analyzer = Analyzer::new(Analysis::named(analysis));
    let path = index_path(Path::new(out), cohort, analysis, backend);
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    let ids: Vec<&str> = loaded.docs.iter().map(|doc| doc.id.as_str()).collect();
    std::fs::write(path.join("ids.txt"), ids.join("\n")).unwrap();
    let asked: Vec<Asked> = loaded
        .questions
        .iter()
        .map(|question| Asked {
            id: question.id.clone(),
            text: question.text.clone(),
            judged: question.judged.clone(),
        })
        .collect();
    std::fs::write(
        path.join("questions.json"),
        serde_json::to_vec(&asked).unwrap(),
    )
    .unwrap();
    let source: u64 = loaded
        .docs
        .iter()
        .map(|doc| (doc.title.len() + doc.text.len()) as u64)
        .sum();
    let line = match backend.as_str() {
        "fts" => {
            let built = fts::build(&path.join("index.db"), &analyzer, &loaded.docs);
            json!({"seconds": built.seconds, "bytes": built.file, "objects": built.objects,
                "frequent_terms": built.frequent_terms, "frequent_seconds": built.frequent_seconds})
        }
        #[cfg(feature = "tantivy")]
        "tantivy" => {
            let built = tant::build(&path.join("tantivy"), &analyzer, &loaded.docs);
            json!({"seconds": built.seconds, "bytes": built.bytes})
        }
        "blocks" => {
            let built = blocks::build(&path.join("index.db"), &analyzer, &loaded.docs);
            json!({"seconds": built.seconds, "bytes": built.file, "objects": built.objects,
                "postings": built.postings, "blocks": built.blocks})
        }
        other => panic!("no backend {other}"),
    };
    emit(json!({
        "step": "build", "cohort": cohort, "analysis": analysis, "backend": backend,
        "documents": loaded.docs.len(), "questions": asked.len(), "source_bytes": source,
        "load_seconds": load_seconds, "index": line, "memory": memory(),
    }));
}

struct Settings {
    plans: Vec<Plan>,
    passes: usize,
    cut: f64,
    typos: bool,
    fuzzy: bool,
}

fn query(args: &[String]) {
    let [
        out,
        cohort,
        analysis,
        backend,
        plans,
        passes,
        cut,
        variant,
        matching,
    ] = args
    else {
        panic!(
            "query <out> <cohort> <analysis> <backend> <plans> <passes> <cut> <clean|typos> <exact|fuzzy>"
        )
    };
    let settings = Settings {
        plans: plans.split(',').map(Plan::named).collect(),
        passes: passes.parse().unwrap(),
        cut: cut.parse().unwrap(),
        typos: variant == "typos",
        fuzzy: matching == "fuzzy",
    };
    let path = index_path(Path::new(out), cohort, analysis, backend);
    let ids: Vec<String> = std::fs::read_to_string(path.join("ids.txt"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut asked: Vec<Asked> =
        serde_json::from_slice(&std::fs::read(path.join("questions.json")).unwrap()).unwrap();
    if settings.typos {
        for question in &mut asked {
            question.text =
                misspell(&question.id, &question.text).unwrap_or_else(|| question.text.clone());
        }
    }
    let analyzer = Analyzer::new(Analysis::named(analysis));
    // What the process holds before the index opens: the ids and the questions.
    let baseline = memory();
    let opened = Instant::now();
    let (mut backend_box, typos): (Box<dyn Backend>, Option<Typos>) = match backend.as_str() {
        "fts" => {
            let index = fts::Fts::open(&path.join("index.db"));
            emit(
                json!({"step": "open", "frequent_terms": index.opened.frequent_terms, "frequent_millis": index.opened.frequent_millis}),
            );
            let typos = settings.fuzzy.then(|| Typos::build(index.connection()));
            if let Some(typos) = &typos {
                emit(
                    json!({"step": "typos", "terms": typos.built.terms, "bytes": typos.built.bytes, "millis": typos.built.millis}),
                );
            }
            (Box::new(index), typos)
        }
        #[cfg(feature = "tantivy")]
        "tantivy" => (Box::new(tant::Tant::open(&path.join("tantivy"))), None),
        "blocks" => (Box::new(blocks::Blocks::open(&path.join("index.db"))), None),
        other => panic!("no backend {other}"),
    };
    let open_millis = opened.elapsed().as_secs_f64() * 1e3;
    // Which of run.sh's alternating runs this process is.
    let round = std::env::var("LAB_ROUND").unwrap_or_default();
    let fst = std::env::var("LAB_TYPOS").unwrap_or_else(|_| "one".to_owned());
    let raw_dir = Path::new(out).join("raw");
    std::fs::create_dir_all(&raw_dir).unwrap();
    let raw_name =
        format!("{cohort}-{analysis}-{backend}-{variant}-{matching}-{cut}-{fst}-r{round}.jsonl");
    let mut raw = std::fs::File::create(raw_dir.join(raw_name)).unwrap();
    let context = json!({"cohort": cohort, "analysis": analysis, "backend": backend, "variant": variant,
        "matching": matching, "cut": settings.cut, "round": round, "fst": fst});
    // A pass unrecorded first, so that every recorded pass finds the index's pages warm.
    for pass in 0..=settings.passes {
        for plan in &settings.plans {
            let ran = ask(
                &mut *backend_box,
                typos.as_ref(),
                &analyzer,
                &asked,
                *plan,
                &settings,
            );
            if pass == 0 {
                continue;
            }
            for (question, result) in asked.iter().zip(&ran) {
                let found: Vec<&str> = result
                    .ids
                    .iter()
                    .map(|id| ids[*id as usize - 1].as_str())
                    .collect();
                let line = json!({"pass": pass, "plan": plan.name(), "question": question.id, "millis": result.millis,
                    "hydrate_millis": result.hydrate_millis, "queries": result.queries, "found": found});
                writeln!(raw, "{line}").unwrap();
            }
            let mut line = summary(&asked, &ran, &ids);
            line["step"] = json!("query");
            line["pass"] = json!(pass);
            line["plan"] = json!(plan.name());
            line["context"] = context.clone();
            emit(line);
        }
    }
    emit(
        json!({"step": "memory", "context": context, "open_millis": open_millis, "baseline": baseline, "memory": memory()}),
    );
}

struct Ran {
    ids: Vec<u32>,
    millis: f64,
    /// Reading the ten documents' key, title and body after their ids.
    hydrate_millis: f64,
    queries: usize,
}

/// Every question through one plan, each timed from its text to its ten ids.
fn ask(
    backend: &mut dyn Backend,
    typos: Option<&Typos>,
    analyzer: &Analyzer,
    asked: &[Asked],
    plan: Plan,
    settings: &Settings,
) -> Vec<Ran> {
    asked
        .iter()
        .map(|question| {
            let started = Instant::now();
            let mut words = plan::words(analyzer, &question.text);
            if let Some(typos) = typos {
                for word in &mut words {
                    let absent = backend.frequency(&word.surface) == 0
                        && word
                            .stem
                            .as_deref()
                            .is_none_or(|stem| backend.frequency(stem) == 0);
                    if absent {
                        word.typos = typos.candidates(&word.surface);
                    }
                }
            }
            let answer = plan::run(backend, plan, words, settings.cut, 10);
            let millis = started.elapsed().as_secs_f64() * 1e3;
            let hydrating = Instant::now();
            std::hint::black_box(backend.hydrate(&answer.ids));
            Ran {
                ids: answer.ids,
                millis,
                hydrate_millis: hydrating.elapsed().as_secs_f64() * 1e3,
                queries: answer.queries,
            }
        })
        .collect()
}

/// Latency percentiles, rate and quality over every question of a plan.
fn summary(asked: &[Asked], ran: &[Ran], ids: &[String]) -> serde_json::Value {
    let mut millis: Vec<f64> = ran.iter().map(|result| result.millis).collect();
    millis.sort_by(f64::total_cmp);
    let at = |share: f64| {
        millis[((millis.len() as f64 * share).ceil() as usize).clamp(1, millis.len()) - 1]
    };
    let mut hydrated: Vec<f64> = ran
        .iter()
        .map(|result| result.millis + result.hydrate_millis)
        .collect();
    hydrated.sort_by(f64::total_cmp);
    let hydrated_at = |share: f64| {
        hydrated[((hydrated.len() as f64 * share).ceil() as usize).clamp(1, hydrated.len()) - 1]
    };
    let total: f64 = millis.iter().sum();
    let (mut ndcg, mut recall, mut mrr) = (0.0, 0.0, 0.0);
    for (question, result) in asked.iter().zip(ran) {
        let found: Vec<&str> = result
            .ids
            .iter()
            .map(|id| ids[*id as usize - 1].as_str())
            .collect();
        let relevant = question.judged.values().filter(|grade| **grade > 0).count();
        let gains: Vec<f64> = found
            .iter()
            .map(|doc| f64::from(*question.judged.get(*doc).unwrap_or(&0)))
            .collect();
        let dcg: f64 = gains
            .iter()
            .enumerate()
            .map(|(rank, gain)| gain / (rank as f64 + 2.0).log2())
            .sum();
        let ideal: f64 = (0..relevant.min(10))
            .map(|rank| 1.0 / (rank as f64 + 2.0).log2())
            .sum();
        ndcg += dcg / ideal;
        recall += gains.iter().filter(|gain| **gain > 0.0).count() as f64 / relevant as f64;
        mrr += gains
            .iter()
            .position(|gain| *gain > 0.0)
            .map_or(0.0, |rank| 1.0 / (rank as f64 + 1.0));
    }
    let n = asked.len() as f64;
    json!({
        "questions": asked.len(),
        "p50_ms": at(0.50), "p90_ms": at(0.90), "p99_ms": at(0.99), "max_ms": millis[millis.len() - 1],
        "mean_ms": total / n, "per_second": n / (total / 1e3),
        "hydrated_p50_ms": hydrated_at(0.50), "hydrated_p99_ms": hydrated_at(0.99),
        "ndcg10": ndcg / n, "recall10": recall / n, "mrr10": mrr / n,
        "empty": ran.iter().filter(|result| result.ids.is_empty()).count(),
        "index_queries": ran.iter().map(|result| result.queries).sum::<usize>() as f64 / n,
    })
}

/// Sets the first 600 Russian passages again, their bodies changed: 100
/// one a commit, then five commits of 100. The cohort must start with the
/// Russian pool, so that its passage n is rowid n.
fn write(args: &[String]) {
    let [corpus, out, cohort, analysis, backend] = args else {
        panic!("write <corpus> <out> <cohort> <analysis> <backend>")
    };
    let round = std::env::var("LAB_ROUND").unwrap_or_default();
    let analyzer = Analyzer::new(Analysis::named(analysis));
    let mut docs = corpus::first_docs(Path::new(corpus), "ru", 600);
    for doc in &mut docs {
        doc.text.push_str(&format!(" обновлено {round}"));
    }
    let path = index_path(Path::new(out), cohort, analysis, backend);
    let mut set: Box<dyn FnMut(&[(u32, &corpus::Doc)])> = match backend.as_str() {
        "fts" => {
            let mut writer = fts::Writer::open(&path.join("index.db"));
            Box::new(move |docs| writer.set(&analyzer, docs))
        }
        #[cfg(feature = "tantivy")]
        "tantivy" => {
            let mut writer = tant::Writer::open(&path.join("tantivy"));
            Box::new(move |docs| writer.set(&analyzer, docs))
        }
        "blocks" => {
            let mut writer = blocks::Writer::open(&path.join("index.db"));
            Box::new(move |docs| writer.set(&analyzer, docs))
        }
        other => panic!("no backend {other}"),
    };
    let numbered: Vec<(u32, &corpus::Doc)> = docs
        .iter()
        .enumerate()
        .map(|(at, doc)| (at as u32 + 1, doc))
        .collect();
    let mut singles: Vec<f64> = numbered[..100]
        .iter()
        .map(|one| {
            let started = Instant::now();
            set(std::slice::from_ref(one));
            started.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    let batches: Vec<f64> = numbered[100..]
        .chunks(100)
        .map(|batch| {
            let started = Instant::now();
            set(batch);
            started.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    singles.sort_by(f64::total_cmp);
    let at = |share: f64| singles[((singles.len() as f64 * share).ceil() as usize) - 1];
    emit(json!({
        "step": "write", "cohort": cohort, "analysis": analysis, "backend": backend, "round": round,
        "single_p50_ms": at(0.50), "single_p90_ms": at(0.90), "single_p99_ms": at(0.99),
        "single_max_ms": singles[singles.len() - 1],
        "single_mean_ms": singles.iter().sum::<f64>() / singles.len() as f64,
        "batches_ms": batches, "memory": memory(),
    }));
}

/// Answers every question by block-max MaxScore and by scoring every
/// posting, and counts the questions whose ten scores differ.
fn check(args: &[String]) {
    let [out, cohort, analysis, plans, cut] = args else {
        panic!("check <out> <cohort> <analysis> <plans> <cut>")
    };
    let path = index_path(Path::new(out), cohort, analysis, "blocks");
    let asked: Vec<Asked> =
        serde_json::from_slice(&std::fs::read(path.join("questions.json")).unwrap()).unwrap();
    let analyzer = Analyzer::new(Analysis::named(analysis));
    let mut index = blocks::Blocks::open(&path.join("index.db"));
    let cut: f64 = cut.parse().unwrap();
    for plan in plans.split(',').map(Plan::named) {
        let (mut differ, mut opened, mut opened_all) = (0, 0, 0);
        let (mut millis, mut millis_all) = (0.0, 0.0);
        for question in &asked {
            let answer = |exhaustive: bool, index: &mut blocks::Blocks| {
                index.exhaustive = exhaustive;
                let started = Instant::now();
                plan::run(index, plan, plan::words(&analyzer, &question.text), cut, 10);
                (
                    started.elapsed().as_secs_f64() * 1e3,
                    index.scores.clone(),
                    index.opened,
                )
            };
            let (took, pruned, blocks_opened) = answer(false, &mut index);
            let (took_all, all, blocks_all) = answer(true, &mut index);
            let same = pruned.len() == all.len()
                && pruned
                    .iter()
                    .zip(&all)
                    .all(|(a, b)| (a - b).abs() <= 1e-9 * b.abs().max(1.0));
            differ += usize::from(!same);
            (opened, opened_all) = (opened + blocks_opened, opened_all + blocks_all);
            (millis, millis_all) = (millis + took, millis_all + took_all);
        }
        let n = asked.len() as f64;
        emit(
            json!({"step": "check", "plan": plan.name(), "cut": cut, "questions": asked.len(),
            "differ": differ, "blocks_opened": opened as f64 / n, "blocks_opened_exhaustive": opened_all as f64 / n,
            "mean_ms": millis / n, "mean_ms_exhaustive": millis_all / n}),
        );
    }
}

/// The process's peak and current resident memory in KiB, on Linux.
fn memory() -> serde_json::Value {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return json!(null);
    };
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().trim_end_matches(" kB").parse::<u64>().ok())
    };
    json!({"hwm_kib": field("VmHWM:"), "rss_kib": field("VmRSS:")})
}

fn emit(line: serde_json::Value) {
    println!("{line}");
}

/// A question with one typo in its longest word of five letters or more,
/// chosen and placed by the question's id: a letter replaced, dropped,
/// doubled or swapped with the next.
pub fn misspell(id: &str, text: &str) -> Option<String> {
    let longest = text
        .split(|c: char| !c.is_alphabetic())
        .filter(|word| word.chars().count() >= 5)
        .max_by_key(|word| word.chars().count())?;
    let letters: Vec<char> = longest.chars().collect();
    let digest = <sha2::Sha256 as sha2::Digest>::digest(id);
    let at = 1 + digest[0] as usize % (letters.len() - 2);
    let mut wrong = letters.clone();
    match digest[1] % 4 {
        0 => wrong[at] = letters[(at + 2) % letters.len()],
        1 => {
            wrong.remove(at);
        }
        2 => wrong.insert(at, letters[at]),
        _ => wrong.swap(at, at + 1),
    }
    if wrong == letters {
        wrong[at] = if letters[at] == letters[0] {
            letters[letters.len() - 1]
        } else {
            letters[0]
        };
    }
    let wrong: String = wrong.into_iter().collect();
    Some(text.replacen(longest, &wrong, 1))
}

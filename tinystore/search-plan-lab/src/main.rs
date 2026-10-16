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
mod compact;
mod corpus;
mod fts;
mod plan;
#[cfg(feature = "tantivy")]
mod tant;
mod typo;
mod vectors;
mod vwrite;

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
        Some("churn") => churn(&args[1..]),
        Some("export") => export(&args[1..]),
        Some("vtruth") => emit(vectors::truth(
            Path::new(&args[1]),
            args[2].parse().unwrap(),
        )),
        Some("vbuild") => vbuild(&args[1..]),
        Some("vquery") => vquery(&args[1..]),
        Some("vhybrid") => vhybrid(&args[1..]),
        Some("vchurn") => vwrite::churn(&args[1..]),
        Some("tokens") => {
            let analyzer = Analyzer::new(Analysis::named(&args[1]));
            for token in analyzer.tokens(&args[2]) {
                println!("{}\t{}", token.surface, token.stem.unwrap_or_default());
            }
        }
        _ => panic!("lab build|query|write|check|churn|tokens"),
    }
}

#[derive(Serialize, Deserialize)]
struct Asked {
    id: String,
    text: String,
    judged: HashMap<String, u8>,
}

/// A cohort's passages and questions as JSON lines, in the order every index
/// numbers them, for a model outside the lab to embed: passage n is document
/// n + 1 of each index.
fn export(args: &[String]) {
    let [corpus, out, cohort] = args else {
        panic!("export <corpus> <out> <cohort>")
    };
    let loaded = corpus::load(Path::new(corpus), cohort);
    let out = Path::new(out);
    std::fs::create_dir_all(out).unwrap();
    let mut docs = std::io::BufWriter::new(std::fs::File::create(out.join("docs.jsonl")).unwrap());
    for doc in &loaded.docs {
        let line = json!({"id": doc.id, "title": doc.title, "text": doc.text});
        writeln!(docs, "{line}").unwrap();
    }
    docs.flush().unwrap();
    let mut questions =
        std::io::BufWriter::new(std::fs::File::create(out.join("questions.jsonl")).unwrap());
    for question in &loaded.questions {
        writeln!(
            questions,
            "{}",
            json!({"id": question.id, "text": question.text})
        )
        .unwrap();
    }
    questions.flush().unwrap();
    emit(
        json!({"step": "export", "cohort": cohort, "documents": loaded.docs.len(), "questions": loaded.questions.len()}),
    );
}

/// `vbuild <vectors> <out> vectors <clusters> <sample> <iterations>`, or
/// `hnsw`, `sqlitevec`, or `vec1 <buckets> <codesize> <sample>`.
fn vbuild(args: &[String]) {
    let (vectors, out) = (Path::new(&args[0]), Path::new(&args[1]));
    std::fs::create_dir_all(out).unwrap();
    let number = |at: usize| -> usize { args[at].parse().unwrap() };
    let mut line = match args[2].as_str() {
        "vectors" => vectors::build(vectors, out, number(3), number(4), number(5)),
        #[cfg(feature = "usearch")]
        "hnsw" => vectors::hnsw::build(vectors, out),
        #[cfg(feature = "sqlitevec")]
        "sqlitevec" => vectors::scan::build(vectors, out),
        #[cfg(feature = "vec1")]
        "vec1" => vectors::ivf1::build(vectors, out, number(3), number(4), number(5)),
        other => panic!("no vector backend {other}"),
    };
    line["memory"] = memory();
    emit(line);
}

/// What a vector arm asks: the words index's ids and judged questions, the
/// questions' vectors and their exact answers, the first `questions` of each.
struct VectorQuestions {
    ids: Vec<String>,
    asked: Vec<Asked>,
    vectors: Vec<f32>,
    truth: Vec<Vec<vectors::Scored>>,
}

fn vector_questions(vectors: &Path, words: &Path, questions: usize) -> VectorQuestions {
    let ids = std::fs::read_to_string(words.join("ids.txt"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut asked: Vec<Asked> =
        serde_json::from_slice(&std::fs::read(words.join("questions.json")).unwrap()).unwrap();
    asked.truncate(questions);
    let mut asked_vectors = vectors::read_all(&vectors.join("questions.f32"));
    asked_vectors.truncate(questions * vectors::DIM);
    let mut truth = vectors::read_truth(vectors);
    truth.truncate(questions);
    VectorQuestions {
        ids,
        asked,
        vectors: asked_vectors,
        truth,
    }
}

/// `vquery <vectors> <out> <words index> <arm> <questions>`: a pass to warm
/// the pages, then one timed, each answer compared with the exact one.
fn vquery(args: &[String]) {
    let [vectors, out, words, arm, questions] = args else {
        panic!("vquery <vectors> <out> <words index> <arm> <questions>")
    };
    let (vectors, out) = (Path::new(vectors), Path::new(out));
    let k: usize = std::env::var("LAB_K").map_or(10, |k| k.parse().unwrap());
    let given = vector_questions(vectors, Path::new(words), questions.parse().unwrap());
    let n = given.asked.len();
    let baseline = memory();
    let opened = Instant::now();
    let mut searcher = vectors::Searcher::open(vectors, out, arm);
    let open_millis = opened.elapsed().as_secs_f64() * 1e3;
    let round = std::env::var("LAB_ROUND").unwrap_or_default();
    let context = json!({"arm": arm, "round": round, "questions": n, "k": k});
    for pass in 0..2 {
        searcher.take_counts();
        let (mut ran, mut recall, mut differ) = (Vec::new(), 0.0, 0);
        for (q, exact) in given.vectors.chunks_exact(vectors::DIM).zip(&given.truth) {
            let started = Instant::now();
            let found = searcher.search(q, k);
            let millis = started.elapsed().as_secs_f64() * 1e3;
            let exact = &exact[..k];
            recall += found
                .iter()
                .filter(|s| exact.iter().any(|e| e.doc == s.doc))
                .count() as f64
                / k as f64;
            let same = found.len() == k
                && found
                    .iter()
                    .zip(exact)
                    .all(|(a, b)| a.doc == b.doc && a.score.to_bits() == b.score.to_bits());
            differ += usize::from(!same);
            ran.push(Ran {
                ids: found.iter().map(|s| s.doc).collect(),
                millis,
                hydrate_millis: 0.0,
                queries: 0,
            });
        }
        let counts = searcher.take_counts();
        if pass == 0 {
            continue;
        }
        let mut line = summary(&given.asked, &ran, &given.ids);
        line["step"] = json!("vquery");
        line["recall_exact"] = json!(recall / n as f64);
        line["differ"] = json!(differ);
        line["read"] = counts.per(n);
        line["context"] = context.clone();
        emit(line);
    }
    emit(
        json!({"step": "memory", "context": context, "open_millis": open_millis, "baseline": baseline, "memory": memory()}),
    );
}

/// Reciprocal rank fusion: each list adds its weight / (60 + rank) to a document.
fn fused(lists: &[(&[u32], f64)], k: usize) -> Vec<u32> {
    let mut scores: HashMap<u32, f64> = HashMap::new();
    for (list, weight) in lists {
        for (rank, doc) in list.iter().enumerate() {
            *scores.entry(*doc).or_default() += weight / (60.0 + rank as f64 + 1.0);
        }
    }
    let mut all: Vec<(u32, f64)> = scores.into_iter().collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    all.into_iter().take(k).map(|(doc, _)| doc).collect()
}

/// A convex combination of the two lists' scores: each list's turned to
/// [0, 1] by its own lowest and highest, a document a list lacks counted 0
/// there, the vectors' weighing `weight` and the words' the rest.
fn convex(words: &[(u32, f64)], vectors: &[(u32, f64)], weight: f64, k: usize) -> Vec<u32> {
    let scaled = |list: &[(u32, f64)]| -> HashMap<u32, f64> {
        let low = list.iter().map(|(_, s)| *s).fold(f64::INFINITY, f64::min);
        let high = list
            .iter()
            .map(|(_, s)| *s)
            .fold(f64::NEG_INFINITY, f64::max);
        let width = if high > low { high - low } else { 1.0 };
        list.iter()
            .map(|(doc, s)| (*doc, (s - low) / width))
            .collect()
    };
    let (by_words, by_vectors) = (scaled(words), scaled(vectors));
    let mut all: Vec<(u32, f64)> = by_words
        .keys()
        .chain(by_vectors.keys())
        .copied()
        .collect::<std::collections::HashSet<u32>>()
        .into_iter()
        .map(|doc| {
            let score = (1.0 - weight) * by_words.get(&doc).copied().unwrap_or(0.0)
                + weight * by_vectors.get(&doc).copied().unwrap_or(0.0);
            (doc, score)
        })
        .collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    all.into_iter().take(k).map(|(doc, _)| doc).collect()
}

/// `vhybrid <vectors> <out> <words index> <arm> <questions>`: the words'
/// hundred best by `or` over the compact blocks, the vectors' hundred best
/// by the arm, and the ten of their fusion, each judged on its own; and the
/// words' hundred scored on their vectors as given, which needs no vector
/// index: fused with the words, and alone.
fn vhybrid(args: &[String]) {
    let [vectors, out, words, arm, questions] = args else {
        panic!("vhybrid <vectors> <out> <words index> <arm> <questions>")
    };
    let (vectors, out, words) = (Path::new(vectors), Path::new(out), Path::new(words));
    let given = vector_questions(vectors, words, questions.parse().unwrap());
    let analyzer = Analyzer::new(Analysis::named("auto"));
    let mut index = compact::Compact::open(&words.join("index.db"));
    let mut searcher = vectors::Searcher::open(vectors, out, arm);
    let mut reader = vectors::Reader::open(out);
    let round = std::env::var("LAB_ROUND").unwrap_or_default();
    let context = json!({"arm": arm, "round": round, "questions": given.asked.len(), "depth": 100});
    for pass in 0..2 {
        let (mut alone, mut near, mut both) = (Vec::new(), Vec::new(), Vec::new());
        let (mut rescored, mut reordered) = (Vec::new(), Vec::new());
        let (mut twice, mut four) = (Vec::new(), Vec::new());
        let mut convexes: Vec<Vec<Ran>> = (0..3).map(|_| Vec::new()).collect();
        for (question, q) in given
            .asked
            .iter()
            .zip(given.vectors.chunks_exact(vectors::DIM))
        {
            let started = Instant::now();
            let found = plan::run(
                &mut index,
                Plan::Or,
                plan::words(&analyzer, &question.text),
                0.05,
                100,
            )
            .ids;
            let bm25: Vec<(u32, f64)> = found
                .iter()
                .copied()
                .zip(index.scores.iter().copied())
                .collect();
            let worded = started.elapsed().as_secs_f64() * 1e3;
            let searched = searcher.search(q, 100);
            let total = started.elapsed().as_secs_f64() * 1e3;
            let nearest: Vec<u32> = searched.iter().map(|s| s.doc).collect();
            let cosines: Vec<(u32, f64)> = searched
                .iter()
                .map(|s| (s.doc, f64::from(s.score)))
                .collect();
            for (ran, weight) in convexes.iter_mut().zip([0.5, 0.7, 0.9]) {
                ran.push(Ran {
                    ids: convex(&bm25, &cosines, weight, 10),
                    millis: total,
                    hydrate_millis: 0.0,
                    queries: 0,
                });
            }
            let top = |list: &[u32]| list.iter().take(10).copied().collect::<Vec<u32>>();
            alone.push(Ran {
                ids: top(&found),
                millis: worded,
                hydrate_millis: 0.0,
                queries: 0,
            });
            near.push(Ran {
                ids: top(&nearest),
                millis: total - worded,
                hydrate_millis: 0.0,
                queries: 0,
            });
            twice.push(Ran {
                ids: fused(&[(&found, 1.0), (&nearest, 2.0)], 10),
                millis: total,
                hydrate_millis: 0.0,
                queries: 0,
            });
            four.push(Ran {
                ids: fused(&[(&found, 1.0), (&nearest, 4.0)], 10),
                millis: total,
                hydrate_millis: 0.0,
                queries: 0,
            });
            both.push(Ran {
                ids: fused(&[(&found, 1.0), (&nearest, 1.0)], 10),
                millis: total,
                hydrate_millis: 0.0,
                queries: 0,
            });
            let again = Instant::now();
            let scored: Vec<u32> = reader.score(q, &found).iter().map(|s| s.doc).collect();
            let millis = worded + again.elapsed().as_secs_f64() * 1e3;
            rescored.push(Ran {
                ids: fused(&[(&found, 1.0), (&scored, 1.0)], 10),
                millis,
                hydrate_millis: 0.0,
                queries: 0,
            });
            reordered.push(Ran {
                ids: top(&scored),
                millis,
                hydrate_millis: 0.0,
                queries: 0,
            });
        }
        if pass == 0 {
            continue;
        }
        let modes = [
            ("words", &alone),
            ("vectors", &near),
            ("hybrid", &both),
            ("hybrid, vectors ×2", &twice),
            ("hybrid, vectors ×4", &four),
            ("convex, vectors 0.5", &convexes[0]),
            ("convex, vectors 0.7", &convexes[1]),
            ("convex, vectors 0.9", &convexes[2]),
            ("words rescored, fused", &rescored),
            ("words rescored", &reordered),
        ];
        // Every question's ten by every mode, for judging them by kind of
        // question and by graded labels outside the lab.
        let raw_dir = out.join("raw");
        std::fs::create_dir_all(&raw_dir).unwrap();
        let name = format!("hybrid-{}.jsonl", arm.replace(':', "-"));
        let mut raw = std::io::BufWriter::new(std::fs::File::create(raw_dir.join(name)).unwrap());
        for (mode, ran) in modes {
            for (question, answer) in given.asked.iter().zip(ran.iter()) {
                let found: Vec<&str> = answer
                    .ids
                    .iter()
                    .map(|id| given.ids[*id as usize - 1].as_str())
                    .collect();
                writeln!(
                    raw,
                    "{}",
                    json!({"mode": mode, "question": question.id, "found": found})
                )
                .unwrap();
            }
            let mut line = summary(&given.asked, ran, &given.ids);
            line["step"] = json!("vhybrid");
            line["mode"] = json!(mode);
            line["context"] = context.clone();
            emit(line);
        }
        raw.flush().unwrap();
    }
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
        "compact" => {
            let built = compact::build(&path.join("index.db"), &analyzer, &loaded.docs);
            json!({"seconds": built.seconds, "bytes": built.file, "objects": built.objects,
                "postings": built.postings, "blocks": built.blocks, "keys": built.keys, "inline": built.inline})
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
        "compact" => (
            Box::new(compact::Compact::open(&path.join("index.db"))),
            None,
        ),
        other => panic!("no backend {other}"),
    };
    let open_millis = opened.elapsed().as_secs_f64() * 1e3;
    // Which of run.sh's alternating runs this process is.
    let round = std::env::var("LAB_ROUND").unwrap_or_default();
    let fst = std::env::var("LAB_TYPOS").unwrap_or_else(|_| "one".to_owned());
    let executor = std::env::var("LAB_EXECUTOR").unwrap_or_else(|_| "cursor".to_owned());
    let mmap = std::env::var("LAB_MMAP").unwrap_or_else(|_| "0".to_owned());
    let raw_dir = Path::new(out).join("raw");
    std::fs::create_dir_all(&raw_dir).unwrap();
    let raw_name = format!(
        "{cohort}-{analysis}-{backend}-{variant}-{matching}-{cut}-{fst}-{executor}-m{mmap}-r{round}.jsonl"
    );
    let mut raw = std::fs::File::create(raw_dir.join(raw_name)).unwrap();
    let context = json!({"cohort": cohort, "analysis": analysis, "backend": backend, "variant": variant,
        "matching": matching, "cut": settings.cut, "round": round, "fst": fst, "executor": executor, "mmap": mmap});
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
        "compact" => {
            let mut writer = compact::Writer::open(&path.join("index.db"));
            Box::new(move |docs| {
                writer.set(&analyzer, docs);
            })
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
/// What the check asks of an index of blocks beyond a plan's answer.
trait Checked: Backend {
    fn set_exhaustive(&mut self, on: bool);
    /// The last query's scores, blocks opened, statements, and time on
    /// directories and on blocks.
    fn last(&self) -> (Vec<f64>, usize, usize, f64, f64);
}

impl Checked for blocks::Blocks {
    fn set_exhaustive(&mut self, on: bool) {
        self.exhaustive = on;
    }

    fn last(&self) -> (Vec<f64>, usize, usize, f64, f64) {
        (
            self.scores.clone(),
            self.opened,
            self.opened,
            self.directory_ms,
            self.opening_ms,
        )
    }
}

impl Checked for compact::Compact {
    fn set_exhaustive(&mut self, on: bool) {
        self.exhaustive = on;
    }

    fn last(&self) -> (Vec<f64>, usize, usize, f64, f64) {
        (
            self.scores.clone(),
            self.opened,
            self.statements,
            self.directory_ms,
            self.opening_ms,
        )
    }
}

fn same_scores(pruned: &[f64], all: &[f64]) -> bool {
    pruned.len() == all.len()
        && pruned
            .iter()
            .zip(all)
            .all(|(a, b)| (a - b).abs() <= 1e-9 * b.abs().max(1.0))
}

/// Answers every question by block-max MaxScore and by scoring every
/// posting, and counts the questions whose ten scores differ.
fn check(args: &[String]) {
    let (out, cohort, analysis, plans, cut, backend) = match args {
        [out, cohort, analysis, plans, cut] => (out, cohort, analysis, plans, cut, "blocks"),
        [out, cohort, analysis, plans, cut, backend] => {
            (out, cohort, analysis, plans, cut, backend.as_str())
        }
        _ => panic!("check <out> <cohort> <analysis> <plans> <cut> [blocks|compact]"),
    };
    let path = index_path(Path::new(out), cohort, analysis, backend);
    let asked: Vec<Asked> =
        serde_json::from_slice(&std::fs::read(path.join("questions.json")).unwrap()).unwrap();
    let analyzer = Analyzer::new(Analysis::named(analysis));
    let mut index: Box<dyn Checked> = match backend {
        "compact" => Box::new(compact::Compact::open(&path.join("index.db"))),
        _ => Box::new(blocks::Blocks::open(&path.join("index.db"))),
    };
    let cut: f64 = cut.parse().unwrap();
    let executor = std::env::var("LAB_EXECUTOR").unwrap_or_else(|_| "cursor".to_owned());
    let ahead = std::env::var("LAB_AHEAD").unwrap_or_else(|_| "1".to_owned());
    for plan in plans.split(',').map(Plan::named) {
        // [pruned, every posting] × (ms, blocks opened, statements, directory ms, opening ms)
        let mut sums = [[0.0f64; 5]; 2];
        let mut differ = 0;
        for question in &asked {
            let mut scores: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
            for (at, exhaustive) in [false, true].into_iter().enumerate() {
                index.set_exhaustive(exhaustive);
                let started = Instant::now();
                plan::run(
                    &mut *index,
                    plan,
                    plan::words(&analyzer, &question.text),
                    cut,
                    10,
                );
                let took = started.elapsed().as_secs_f64() * 1e3;
                let (found, opened, statements, directory_ms, opening_ms) = index.last();
                for (sum, value) in sums[at].iter_mut().zip([
                    took,
                    opened as f64,
                    statements as f64,
                    directory_ms,
                    opening_ms,
                ]) {
                    *sum += value;
                }
                scores[at] = found;
            }
            differ += usize::from(!same_scores(&scores[0], &scores[1]));
        }
        let n = asked.len() as f64;
        let mean = |at: usize, field: usize| sums[at][field] / n;
        emit(
            json!({"step": "check", "backend": backend, "plan": plan.name(), "cut": cut, "executor": executor,
            "ahead": ahead, "questions": asked.len(), "differ": differ,
            "mean_ms": mean(0, 0), "mean_ms_exhaustive": mean(1, 0),
            "blocks_opened": mean(0, 1), "blocks_opened_exhaustive": mean(1, 1), "statements": mean(0, 2),
            "directory_ms": mean(0, 3), "opening_ms": mean(0, 4), "opening_ms_exhaustive": mean(1, 4)}),
        );
    }
}

/// A long mix on a compact index: of every hundred operations 70 searches,
/// 25 sets of a judged passage with its body changed and 5 deletes, the
/// head packed every `pack_every` writes (0, never), every `check_every`th
/// search checked against scoring every posting. A line every 5,000
/// operations says what the searches and sets cost meanwhile.
/// `churn <corpus> <out> <cohort> <analysis> <ops> <pack_every> <check_every> <seed> [<pack_keys> <purge_at>]`:
/// with `pack_keys`, the head is packed that many of its rows a commit, keys
/// whole, until it is empty, and with `purge_at`, once that share of the documents is gone, a
/// purge takes them out of the blocks in steps of as many keys.
fn churn(args: &[String]) {
    let (args, steps) = args.split_at(8.min(args.len()));
    let [
        corpus,
        out,
        cohort,
        analysis,
        ops,
        pack_every,
        check_every,
        seed,
    ] = args
    else {
        panic!(
            "churn <corpus> <out> <cohort> <analysis> <ops> <pack_every> <check_every> <seed> [<pack_keys> <purge_at>]"
        )
    };
    let pack_keys: usize = steps.first().map_or(0, |keys| keys.parse().unwrap());
    let purge_at: f64 = steps.get(1).map_or(0.0, |share| share.parse().unwrap());
    let (ops, pack_every, check_every): (usize, usize, usize) = (
        ops.parse().unwrap(),
        pack_every.parse().unwrap(),
        check_every.parse().unwrap(),
    );
    let mut rng: u64 = seed.parse().unwrap();
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let path = index_path(Path::new(out), cohort, analysis, "compact");
    let asked: Vec<Asked> =
        serde_json::from_slice(&std::fs::read(path.join("questions.json")).unwrap()).unwrap();
    let pool = corpus::load(Path::new(corpus), "mixed").docs;
    let analyzer = Analyzer::new(Analysis::named(analysis));
    let mut writer = compact::Writer::open(&path.join("index.db"));
    let mut reader = compact::Compact::open(&path.join("index.db"));
    reader.shared_gone = true;
    let context = json!({"cohort": cohort, "pack_every": pack_every, "check_every": check_every, "seed": seed,
        "pack_keys": pack_keys, "purge_at": purge_at,
        "executor": std::env::var("LAB_EXECUTOR").unwrap_or_else(|_| "cursor".to_owned())});
    // Each commit that held the writer to pack or purge, and the purges done.
    let (mut held, mut purges, mut purge_spent) = (Vec::new(), 0usize, 0.0f64);
    let mut spent: [Vec<f64>; 4] = Default::default(); // or, pruned-or, set, delete
    let (mut searches, mut writes, mut differ, mut checked) = (0usize, 0usize, 0usize, 0usize);
    let mut packs: Vec<f64> = Vec::new();
    let quantile = |values: &mut Vec<f64>, share: f64| -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.sort_by(f64::total_cmp);
        values[((values.len() as f64 * share).ceil() as usize).clamp(1, values.len()) - 1]
    };
    for op in 1..=ops {
        let roll = next() % 100;
        if roll < 70 {
            let question = &asked[(next() % asked.len() as u64) as usize];
            let plan = if op % 2 == 0 {
                Plan::Or
            } else {
                Plan::PrunedOr
            };
            let words = plan::words(&analyzer, &question.text);
            let started = Instant::now();
            plan::run(&mut reader, plan, words.clone(), 0.05, 10);
            spent[usize::from(plan == Plan::PrunedOr)].push(started.elapsed().as_secs_f64() * 1e3);
            searches += 1;
            if check_every > 0 && searches % check_every == 0 {
                let pruned = reader.scores.clone();
                reader.exhaustive = true;
                plan::run(&mut reader, plan, words, 0.05, 10);
                reader.exhaustive = false;
                differ += usize::from(!same_scores(&pruned, &reader.scores));
                checked += 1;
            }
        } else {
            let doc = &pool[(next() % pool.len() as u64) as usize];
            let started = Instant::now();
            let gone = if roll < 95 {
                let changed = corpus::Doc {
                    id: doc.id.clone(),
                    title: doc.title.clone(),
                    text: format!("{} v{op}", doc.text),
                };
                let gone = writer.set(&analyzer, &[(0, &changed)]);
                spent[2].push(started.elapsed().as_secs_f64() * 1e3);
                gone
            } else {
                let gone = writer.delete(&doc.id);
                spent[3].push(started.elapsed().as_secs_f64() * 1e3);
                gone
            };
            reader.forget(&gone);
            writes += 1;
            if pack_every > 0 && writes % pack_every == 0 {
                if pack_keys == 0 {
                    let packed = writer.pack();
                    packs.push(packed.millis);
                    held.push(packed.millis);
                    emit(
                        json!({"step": "pack", "context": context, "op": op, "documents": packed.documents,
                        "keys": packed.keys, "postings": packed.postings, "millis": packed.millis}),
                    );
                } else {
                    let (mut total, mut keys, mut postings) = (0.0, 0, 0);
                    while writer.head_rows() > 0 {
                        let packed = writer.pack_step(pack_keys);
                        held.push(packed.millis);
                        total += packed.millis;
                        keys += packed.keys;
                        postings += packed.postings;
                    }
                    packs.push(total);
                    emit(
                        json!({"step": "pack", "context": context, "op": op, "keys": keys,
                        "postings": postings, "millis": total}),
                    );
                }
                let documents = reader.documents() as f64;
                if purge_at > 0.0
                    && !writer.purging()
                    && reader.gone_count() as f64 >= purge_at * documents
                {
                    let started = Instant::now();
                    let keys = writer.purge_begin(&analyzer);
                    let read = started.elapsed().as_secs_f64() * 1e3;
                    let mut total = read;
                    while writer.purging() {
                        let step = writer.purge_step(pack_keys.max(1));
                        held.push(step.millis);
                        total += step.millis;
                        reader.purged(&step.purged);
                    }
                    purges += 1;
                    purge_spent += total;
                    emit(
                        json!({"step": "purge", "context": context, "op": op, "keys": keys, "read_ms": read,
                        "millis": total}),
                    );
                }
            }
        }
        if op % 5000 == 0 {
            let head = writer.head_rows();
            emit(json!({"step": "churn", "context": context, "op": op,
                "or_p50_ms": quantile(&mut spent[0], 0.5), "or_p99_ms": quantile(&mut spent[0], 0.99),
                "pruned_p50_ms": quantile(&mut spent[1], 0.5), "pruned_p99_ms": quantile(&mut spent[1], 0.99),
                "set_p50_ms": quantile(&mut spent[2], 0.5), "set_p99_ms": quantile(&mut spent[2], 0.99),
                "delete_p50_ms": quantile(&mut spent[3], 0.5),
                "head_rows": head, "gone": reader.gone_count(), "norms_bytes": reader.norms_bytes(),
                "held_p99_ms": quantile(&mut held, 0.99), "held_max_ms": held.iter().copied().fold(0.0, f64::max),
                "held_commits": held.len(), "purges": purges, "purge_ms": purge_spent,
                "checked": checked, "differ": differ}));
            held.clear();
            for values in &mut spent {
                values.clear();
            }
        }
    }
    emit(
        json!({"step": "churned", "context": context, "ops": ops, "searches": searches, "writes": writes,
        "checked": checked, "differ": differ, "packs": packs.len(),
        "pack_mean_ms": if packs.is_empty() { 0.0 } else { packs.iter().sum::<f64>() / packs.len() as f64 },
        "pack_max_ms": packs.iter().copied().fold(0.0, f64::max), "memory": memory()}),
    );
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

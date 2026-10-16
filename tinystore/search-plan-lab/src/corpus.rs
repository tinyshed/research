//! NoMIRACL's judged passages and dev questions, fetched by fetch.sh. A
//! cohort is one language or several: its passages, each id prefixed with its
//! language so that two languages never share one, and the first 200
//! questions of each language by the SHA-256 of their id, those whose judged
//! passages are all present.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::Path;

use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub struct Doc {
    pub id: String,
    pub title: String,
    pub text: String,
}

pub struct Question {
    pub id: String,
    pub text: String,
    /// Judged passages by id, 1 relevant and 0 not.
    pub judged: HashMap<String, u8>,
}

pub struct Cohort {
    pub docs: Vec<Doc>,
    pub questions: Vec<Question>,
}

#[derive(Deserialize)]
struct Line {
    docid: String,
    title: String,
    text: String,
}

pub const QUESTIONS_EACH: usize = 200;

/// The languages of a cohort: one, or `mixed`, Russian, English and Chinese.
pub fn languages(cohort: &str) -> Vec<&'static str> {
    match cohort {
        "mixed" => vec!["ru", "en", "zh"],
        "ru" => vec!["ru"],
        "en" => vec!["en"],
        "zh" => vec!["zh"],
        "de" => vec!["de"],
        "fr" => vec!["fr"],
        "es" => vec!["es"],
        "ja" => vec!["ja"],
        "th" => vec!["th"],
        other => panic!("no cohort {other}"),
    }
}

fn folder(language: &str) -> &'static str {
    match language {
        "ru" => "russian",
        "en" => "english",
        "zh" => "chinese",
        "de" => "german",
        "fr" => "french",
        "es" => "spanish",
        "ja" => "japanese",
        "th" => "thai",
        other => panic!("no language {other}"),
    }
}

pub fn load(corpus: &Path, cohort: &str) -> Cohort {
    let mut docs = Vec::new();
    let mut questions = Vec::new();
    for language in languages(cohort) {
        let dir = corpus.join(folder(language));
        let start = docs.len();
        read_docs(&dir.join("corpus.jsonl.gz"), language, &mut docs);
        let present: HashSet<&str> = docs[start..].iter().map(|doc| doc.id.as_str()).collect();
        questions.extend(read_questions(&dir, language, &present));
    }
    Cohort { docs, questions }
}

/// Passages in the file's order; an id seen again is the same passage again.
fn read_docs(path: &Path, language: &str, docs: &mut Vec<Doc>) {
    let file =
        std::fs::File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut seen = HashSet::new();
    for line in BufReader::new(GzDecoder::new(file)).lines() {
        let line: Line = serde_json::from_str(&line.unwrap()).unwrap();
        let id = format!("{language}:{}", line.docid);
        if seen.insert(id.clone()) {
            docs.push(Doc {
                id,
                title: line.title,
                text: line.text,
            });
        }
    }
}

fn read_questions(dir: &Path, language: &str, present: &HashSet<&str>) -> Vec<Question> {
    let mut judged: HashMap<String, HashMap<String, u8>> = HashMap::new();
    for line in std::fs::read_to_string(dir.join("qrels/dev.relevant.tsv"))
        .unwrap()
        .lines()
    {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [question, _, doc, grade] = fields[..] else {
            continue;
        };
        let grade: u8 = grade.parse().unwrap();
        judged
            .entry(question.to_owned())
            .or_default()
            .insert(format!("{language}:{doc}"), grade);
    }
    let mut questions: Vec<Question> = std::fs::read_to_string(dir.join("topics/dev.relevant.tsv"))
        .unwrap()
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(id, text)| {
            let judged = judged.get(id)?.clone();
            let complete = judged.keys().all(|doc| present.contains(doc.as_str()));
            let any_relevant = judged.values().any(|grade| *grade > 0);
            (complete && any_relevant).then(|| Question {
                id: format!("{language}:{id}"),
                text: text.to_owned(),
                judged,
            })
        })
        .collect();
    questions.sort_by_cached_key(|question| {
        hex(&Sha256::digest(question.id.split_once(':').unwrap().1))
    });
    questions.truncate(QUESTIONS_EACH);
    questions
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

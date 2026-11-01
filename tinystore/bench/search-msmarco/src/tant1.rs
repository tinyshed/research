//! Tantivy over the one field, as the lab's tant.rs indexes four: the tokens
//! split on spaces, frequencies and positions kept, `id` pid + 1 as a fast
//! field, nothing stored. A keyed index, the one H2.3's writer sets, also
//! indexes `key`, the pid whole, for a set to delete by; its terms and
//! postings would sit in the same files as the text's, so the index whose
//! space and reads are measured has none.
//! Tantivy's BM25 is Lucene's, k1 1.2 and b 0.75 fixed in its source, with a
//! field's length kept in one byte.
//!
//! One indexing thread writes segments in pid order, and they are merged into
//! one in that order, so that a document's number follows its pid as in every
//! other engine of the round; `build` checks it.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use tantivy::collector::{Count, TopDocs};
use tantivy::columnar::Column;
use tantivy::query::{BooleanQuery, Query, TermQuery};
use tantivy::schema::{FAST, Field, IndexRecordOption, STRING, Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::{TextAnalyzer, WhitespaceTokenizer};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, TantivyDocument, Term};

pub struct Fields {
    pub b: Field,
    pub id: Field,
    pub key: Option<Field>,
}

pub fn schema(keyed: bool) -> Schema {
    let mut builder = Schema::builder();
    let text = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("ws")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    builder.add_text_field("b", text);
    builder.add_u64_field("id", FAST);
    if keyed {
        builder.add_text_field("key", STRING);
    }
    builder.build()
}

pub fn fields_of(schema: &Schema) -> Fields {
    Fields {
        b: schema.get_field("b").unwrap(),
        id: schema.get_field("id").unwrap(),
        key: schema.get_field("key").ok(),
    }
}

pub fn register(index: &Index) {
    index
        .tokenizers()
        .register("ws", TextAnalyzer::from(WhitespaceTokenizer::default()));
}

pub fn document(fields: &Fields, pid: u64, tokens: &str) -> TantivyDocument {
    let mut document = TantivyDocument::default();
    document.add_text(fields.b, tokens);
    document.add_u64(fields.id, pid + 1);
    if let Some(key) = fields.key {
        document.add_text(key, pid.to_string());
    }
    document
}

pub fn build(dir: &Path, index_dir: &Path, budget_mib: usize, keyed: bool) -> Value {
    let started = Instant::now();
    std::fs::create_dir_all(index_dir).unwrap();
    let schema = schema(keyed);
    let fields = fields_of(&schema);
    let index = Index::create_in_dir(index_dir, schema).unwrap();
    register(&index);
    let mut writer: IndexWriter<TantivyDocument> =
        index.writer_with_num_threads(1, budget_mib << 20).unwrap();
    let input = BufReader::with_capacity(1 << 22, File::open(dir.join("tokens.tsv")).unwrap());
    let mut rows = 0u64;
    for line in input.lines() {
        let line = line.unwrap();
        let (pid, tokens) = line.split_once('\t').unwrap();
        writer.add_document(document(&fields, pid.parse().unwrap(), tokens)).unwrap();
        rows += 1;
        if rows % 1_000_000 == 0 {
            eprintln!("tantivy {rows} documents, {:.0} s", started.elapsed().as_secs_f64());
        }
    }
    writer.commit().unwrap();
    let indexed = started.elapsed().as_secs_f64();
    let mut segments: Vec<(u64, tantivy::index::SegmentId)> = {
        let reader = index.reader().unwrap();
        reader
            .searcher()
            .segment_readers()
            .iter()
            .map(|segment| {
                let ids: Column<u64> = segment.fast_fields().u64("id").unwrap();
                (ids.min_value(), segment.segment_id())
            })
            .collect()
    };
    segments.sort();
    let merged_from = segments.len();
    if segments.len() > 1 {
        let ordered: Vec<tantivy::index::SegmentId> = segments.iter().map(|(_, id)| *id).collect();
        writer.merge(&ordered).wait().unwrap();
    }
    writer.wait_merging_threads().unwrap();
    let seconds = started.elapsed().as_secs_f64();
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();
    let mut in_order = true;
    let mut postings = 0u64;
    for segment in searcher.segment_readers() {
        let ids: Column<u64> = segment.fast_fields().u64("id").unwrap();
        for doc in 0..segment.max_doc() {
            in_order &= ids.first(doc) == Some(u64::from(doc) + 1);
        }
        let inverted = segment.inverted_index(fields.b).unwrap();
        let mut stream = inverted.terms().stream().unwrap();
        while let Some((_, info)) = stream.next() {
            postings += u64::from(info.doc_freq);
        }
    }
    let segments_after = searcher.segment_readers().len();
    json!({"step": "build", "engine": "tantivy", "keyed": keyed, "documents": rows, "index_seconds": indexed, "seconds": seconds,
        "segments_merged": merged_from, "segments": segments_after, "ids_follow_pids": in_order,
        "postings": postings, "files": crate::sizes::files(index_dir), "memory": crate::memory()})
}

pub struct Reader {
    _reader: IndexReader,
    searcher: Searcher,
    fields: Fields,
    ids: Vec<Column<u64>>,
    /// Scores every matching document instead of skipping, for the check.
    pub exhaustive: bool,
}

impl Reader {
    pub fn open(index_dir: &Path, reload: ReloadPolicy) -> Reader {
        let index = Index::open_in_dir(index_dir).unwrap();
        register(&index);
        let fields = fields_of(&index.schema());
        let reader: IndexReader = index.reader_builder().reload_policy(reload).try_into().unwrap();
        let searcher = reader.searcher();
        let ids = searcher
            .segment_readers()
            .iter()
            .map(|segment| segment.fast_fields().u64("id").unwrap())
            .collect();
        Reader {
            _reader: reader,
            searcher,
            fields,
            ids,
            exhaustive: false,
        }
    }

    /// Takes the reader's latest searcher when a reload has given it a new one.
    pub fn refresh_if_reloaded(&mut self) {
        let latest = self._reader.searcher();
        if latest.generation().generation_id() == self.searcher.generation().generation_id() {
            return;
        }
        self.searcher = latest;
        self.ids = self
            .searcher
            .segment_readers()
            .iter()
            .map(|segment| segment.fast_fields().u64("id").unwrap())
            .collect();
    }

    /// The best `k` documents for any of `terms`, with their scores. With
    /// `exhaustive`, a count beside the top ten makes Tantivy score every
    /// document it matches instead of pruning by block-max WAND.
    pub fn top(&mut self, terms: &[String], k: usize) -> Vec<(f64, u32)> {
        if terms.is_empty() {
            return Vec::new();
        }
        let clauses: Vec<Box<dyn Query>> = terms
            .iter()
            .map(|term| {
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.b, term),
                    IndexRecordOption::WithFreqs,
                )) as Box<dyn Query>
            })
            .collect();
        let query = BooleanQuery::union(clauses);
        let collector = TopDocs::with_limit(k).order_by_score();
        let found = if self.exhaustive {
            self.searcher.search(&query, &(collector, Count)).unwrap().0
        } else {
            self.searcher.search(&query, &collector).unwrap()
        };
        found
            .into_iter()
            .map(|(score, address)| {
                let id = self.ids[address.segment_ord as usize].first(address.doc_id).unwrap() as u32;
                (f64::from(score), id)
            })
            .collect()
    }
}

/// A set deletes its key's document and adds the new one, and returns once
/// they commit; a reader sees them after its next reload.
pub struct Writer {
    writer: IndexWriter<TantivyDocument>,
    fields: Fields,
}

impl Writer {
    pub fn open(index_dir: &Path) -> Writer {
        let index = Index::open_in_dir(index_dir).unwrap();
        register(&index);
        let fields = fields_of(&index.schema());
        assert!(fields.key.is_some(), "a writer sets a keyed index");
        let writer = index.writer_with_num_threads(1, 64 << 20).unwrap();
        Writer { writer, fields }
    }

    pub fn set(&mut self, rows: &[(u64, String)]) {
        for (pid, tokens) in rows {
            self.writer
                .delete_term(Term::from_field_text(self.fields.key.unwrap(), &pid.to_string()));
            self.writer.add_document(document(&self.fields, *pid, tokens)).unwrap();
        }
        self.writer.commit().unwrap();
    }
}

//! The control: the same tokens in Tantivy, split on spaces as FTS5 splits
//! them, the same four fields and weights, the same plans. A plan's `or` is
//! one union of term queries, the shape Tantivy's block-max WAND takes; an
//! `and` is a conjunction of each word's union.
//!
//! The collection owns its documents as FTS5's does: each one's key, indexed
//! whole for a set to delete by, title and body are stored fields, and a set
//! is visible once its commit and the reader's reload return.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use tantivy::collector::TopDocs;
use tantivy::columnar::Column;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery};
use tantivy::schema::document::Value;
use tantivy::schema::{
    FAST, Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
};
use tantivy::tokenizer::{TextAnalyzer, WhitespaceTokenizer};
use tantivy::{
    DocAddress, Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, TantivyDocument, Term,
};

use crate::analyze::Analyzer;
use crate::corpus::Doc;
use crate::fts::columns;
use crate::plan::{Backend, Word};

struct Fields {
    t: Field,
    ts: Field,
    b: Field,
    bs: Field,
    id: Field,
    key: Field,
    title: Field,
    body: Field,
}

fn schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let text = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("ws")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let fields = Fields {
        t: builder.add_text_field("t", text.clone()),
        ts: builder.add_text_field("ts", text.clone()),
        b: builder.add_text_field("b", text.clone()),
        bs: builder.add_text_field("bs", text),
        id: builder.add_u64_field("id", FAST),
        key: builder.add_text_field("key", STRING | STORED),
        title: builder.add_text_field("title", STORED),
        body: builder.add_text_field("body", STORED),
    };
    (builder.build(), fields)
}

fn register(index: &Index) {
    index
        .tokenizers()
        .register("ws", TextAnalyzer::from(WhitespaceTokenizer::default()));
}

pub struct Built {
    pub seconds: f64,
    pub bytes: u64,
}

fn document(fields: &Fields, analyzer: &Analyzer, id: u64, doc: &Doc) -> TantivyDocument {
    let (t, ts) = columns(analyzer, &doc.title);
    let (b, bs) = columns(analyzer, &doc.text);
    let mut document = TantivyDocument::default();
    document.add_text(fields.t, &t);
    document.add_text(fields.ts, &ts);
    document.add_text(fields.b, &b);
    document.add_text(fields.bs, &bs);
    document.add_u64(fields.id, id);
    document.add_text(fields.key, &doc.id);
    document.add_text(fields.title, &doc.title);
    document.add_text(fields.body, &doc.text);
    document
}

/// A collection's writer: a set deletes its key's document and adds the new
/// one, and returns once they commit and the reader sees them.
pub struct Writer {
    writer: IndexWriter<TantivyDocument>,
    reader: IndexReader,
    fields: Fields,
}

impl Writer {
    pub fn open(dir: &Path) -> Writer {
        let index = Index::open_in_dir(dir).unwrap();
        register(&index);
        let (_, fields) = schema();
        let writer = index.writer_with_num_threads(1, 64 << 20).unwrap();
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .unwrap();
        Writer {
            writer,
            reader,
            fields,
        }
    }

    pub fn set(&mut self, analyzer: &Analyzer, docs: &[(u32, &Doc)]) {
        for (id, doc) in docs {
            self.writer
                .delete_term(Term::from_field_text(self.fields.key, &doc.id));
            self.writer
                .add_document(document(&self.fields, analyzer, u64::from(*id), doc))
                .unwrap();
        }
        self.writer.commit().unwrap();
        self.reader.reload().unwrap();
    }
}

/// Indexes every document, commits once and merges every segment into one,
/// as FTS5's index is optimized.
pub fn build(dir: &Path, analyzer: &Analyzer, docs: &[Doc]) -> Built {
    let started = Instant::now();
    std::fs::create_dir_all(dir).unwrap();
    let (schema, fields) = schema();
    let index = Index::create_in_dir(dir, schema).unwrap();
    register(&index);
    let mut writer = index
        .writer_with_num_threads::<TantivyDocument>(1, 256 << 20)
        .unwrap();
    for (at, doc) in docs.iter().enumerate() {
        writer
            .add_document(document(&fields, analyzer, at as u64 + 1, doc))
            .unwrap();
    }
    writer.commit().unwrap();
    let segments = index.searchable_segment_ids().unwrap();
    if segments.len() > 1 {
        writer.merge(&segments).wait().unwrap();
    }
    writer.wait_merging_threads().unwrap();
    let seconds = started.elapsed().as_secs_f64();
    let bytes = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();
    Built { seconds, bytes }
}

pub struct Tant {
    _reader: IndexReader,
    searcher: Searcher,
    fields: Fields,
    ids: Vec<Column<u64>>,
    documents: u64,
    /// Where the documents the last queries found are, for hydrate.
    found: HashMap<u32, DocAddress>,
}

impl Tant {
    pub fn open(dir: &Path) -> Tant {
        let index = Index::open_in_dir(dir).unwrap();
        register(&index);
        let (_, fields) = schema();
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .unwrap();
        let searcher = reader.searcher();
        let ids = searcher
            .segment_readers()
            .iter()
            .map(|segment| segment.fast_fields().u64("id").unwrap())
            .collect();
        let documents = searcher.num_docs();
        Tant {
            _reader: reader,
            searcher,
            fields,
            ids,
            documents,
            found: HashMap::new(),
        }
    }

    fn term(&self, field: Field, text: &str, boost: f32) -> Box<dyn Query> {
        let query = TermQuery::new(
            Term::from_field_text(field, text),
            IndexRecordOption::WithFreqs,
        );
        Box::new(BoostQuery::new(Box::new(query), boost))
    }

    /// A word's forms with FTS5's weights: its surface in t and b, its stem in
    /// ts and bs and in t and b, its typos anywhere.
    fn forms(&self, word: &Word) -> Vec<Box<dyn Query>> {
        let f = &self.fields;
        let stem = word.stem.as_deref().unwrap_or(&word.surface);
        let mut forms = vec![
            self.term(f.t, &word.surface, 2.0),
            self.term(f.b, &word.surface, 1.0),
            self.term(f.ts, stem, 1.5),
            self.term(f.bs, stem, 0.75),
        ];
        if word.stem.is_some() {
            forms.push(self.term(f.t, stem, 2.0));
            forms.push(self.term(f.b, stem, 1.0));
        }
        for typo in &word.typos {
            for (field, boost) in [(f.t, 2.0), (f.ts, 1.5), (f.b, 1.0), (f.bs, 0.75)] {
                forms.push(self.term(field, typo, boost));
            }
        }
        forms
    }
}

impl Backend for Tant {
    fn documents(&self) -> u64 {
        self.documents
    }

    /// The most documents one field holds the term in: FTS5 counts a
    /// document once across its columns, and a sum would count it twice.
    fn frequency(&mut self, term: &str) -> u64 {
        let f = &self.fields;
        [f.t, f.ts, f.b, f.bs]
            .into_iter()
            .map(|field| {
                self.searcher
                    .doc_freq(&Term::from_field_text(field, term))
                    .unwrap()
            })
            .max()
            .unwrap_or(0)
    }

    fn top(&mut self, words: &[Word], every: bool, k: usize) -> Vec<u32> {
        if words.is_empty() {
            return Vec::new();
        }
        let query: Box<dyn Query> = if every {
            let each = words.iter().map(|word| {
                (
                    Occur::Must,
                    Box::new(BooleanQuery::union(self.forms(word))) as Box<dyn Query>,
                )
            });
            Box::new(BooleanQuery::new(each.collect()))
        } else {
            Box::new(BooleanQuery::union(
                words.iter().flat_map(|word| self.forms(word)).collect(),
            ))
        };
        let found = self
            .searcher
            .search(&query, &TopDocs::with_limit(k).order_by_score())
            .unwrap();
        if self.found.len() > 1000 {
            self.found.clear();
        }
        found
            .into_iter()
            .map(|(_, address)| {
                let id = self.ids[address.segment_ord as usize]
                    .first(address.doc_id)
                    .unwrap() as u32;
                self.found.insert(id, address);
                id
            })
            .collect()
    }

    fn hydrate(&mut self, ids: &[u32]) -> usize {
        let f = &self.fields;
        ids.iter()
            .map(|id| {
                let document: TantivyDocument = self.searcher.doc(self.found[id]).unwrap();
                [f.key, f.title, f.body]
                    .into_iter()
                    .map(|field| {
                        document
                            .get_first(field)
                            .and_then(|value| value.as_str())
                            .map_or(0, str::len)
                    })
                    .sum::<usize>()
            })
            .sum()
    }
}

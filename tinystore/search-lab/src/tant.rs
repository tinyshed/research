use crate::analyzer::Analyzer;
use tantivy::{
    Index, IndexReader, IndexWriter, Term,
    collector::{DocSetCollector, TopDocs},
    doc,
    query::{BooleanQuery, Occur, Query, TermQuery},
    schema::*,
    tokenizer::{Token, TokenStream, Tokenizer},
};
#[derive(Clone)]
struct Shared(Analyzer);
struct Stream {
    tokens: std::vec::IntoIter<crate::analyzer::Token>,
    token: Token,
}
impl Tokenizer for Shared {
    type TokenStream<'a> = Stream;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> Stream {
        Stream {
            tokens: self.0.tokens(text).into_iter(),
            token: Token::default(),
        }
    }
}
impl TokenStream for Stream {
    fn advance(&mut self) -> bool {
        if let Some(t) = self.tokens.next() {
            self.token.text = t.text;
            self.token.offset_from = t.start;
            self.token.offset_to = t.end;
            self.token.position = t.position;
            true
        } else {
            false
        }
    }
    fn token(&self) -> &Token {
        &self.token
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}
pub struct TextIndex {
    pub index: Index,
    pub writer: IndexWriter,
    pub reader: IndexReader,
    id: Field,
    title: Field,
    body: Field,
    analyzer: Analyzer,
}
impl TextIndex {
    pub fn new(path: &str, mode: &str) -> Self {
        std::fs::create_dir(path).unwrap();
        let mut s = Schema::builder();
        let id = s.add_u64_field("id", INDEXED | FAST);
        let options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("shared")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
        let title = s.add_text_field("title", options.clone());
        let body = s.add_text_field("body", options);
        let index = Index::create_in_dir(path, s.build()).unwrap();
        let analyzer = Analyzer::new(mode);
        index
            .tokenizers()
            .register("shared", Shared(analyzer.clone()));
        let writer = index.writer_with_num_threads(1, 32_000_000).unwrap();
        let reader = index.reader().unwrap();
        Self {
            index,
            writer,
            reader,
            id,
            title,
            body,
            analyzer,
        }
    }
    pub fn add(&mut self, d: &crate::experiment::Doc) {
        self.writer
            .add_document(
                doc!(self.id=>d.id,self.title=>d.title.as_str(),self.body=>d.body.as_str()),
            )
            .unwrap();
    }
    pub fn delete(&mut self, id: u64) {
        self.writer.delete_term(Term::from_field_u64(self.id, id));
    }
    pub fn commit(&mut self) {
        self.writer.commit().unwrap();
        self.reader.reload().unwrap();
    }
    fn query(&self, q: &str) -> BooleanQuery {
        // Match the literal OR query's per-ICU-word tokenizer calls in FTS5.
        let words = Analyzer::new("icu").tokens(q);
        let mut terms = std::collections::BTreeSet::new();
        for w in words {
            for t in self.analyzer.tokens(&q[w.start..w.end]) {
                terms.insert(t.text);
            }
        }
        let mut qs: Vec<(Occur, Box<dyn Query>)> = vec![];
        for t in terms {
            for field in [self.title, self.body] {
                qs.push((
                    Occur::Should,
                    Box::new(TermQuery::new(
                        Term::from_field_text(field, &t),
                        IndexRecordOption::WithFreqsAndPositions,
                    )),
                ));
            }
        }
        BooleanQuery::new(qs)
    }
    pub fn search(&self, q: &str, all: bool) -> Vec<u64> {
        self.search_filtered(q, all, None)
    }
    pub fn search_filtered(&self, q: &str, all: bool, filter: Option<Vec<u64>>) -> Vec<u64> {
        let s = self.reader.searcher();
        let query = self.query(q);
        let q: Box<dyn Query> = if let Some(ids) = filter {
            Box::new(BooleanQuery::new(vec![
                (Occur::Must, Box::new(query)),
                (
                    Occur::Must,
                    Box::new(tantivy::query::TermSetQuery::new(
                        ids.into_iter().map(|id| Term::from_field_u64(self.id, id)),
                    )),
                ),
            ]))
        } else {
            Box::new(query)
        };
        let fields: Vec<_> = s
            .segment_readers()
            .iter()
            .map(|r| r.fast_fields().u64("id").unwrap())
            .collect();
        let addrs = if all {
            s.search(q.as_ref(), &DocSetCollector)
                .unwrap()
                .into_iter()
                .collect()
        } else {
            s.search(q.as_ref(), &TopDocs::with_limit(10).order_by_score())
                .unwrap()
                .into_iter()
                .map(|(_, a)| a)
                .collect::<Vec<_>>()
        };
        let mut ids: Vec<_> = addrs
            .into_iter()
            .map(|a| fields[a.segment_ord as usize].first(a.doc_id).unwrap())
            .collect();
        if all {
            ids.sort_unstable()
        }
        ids
    }
    pub fn reopen(&mut self, path: &str) {
        let i = Index::open_in_dir(path).unwrap();
        i.tokenizers()
            .register("shared", Shared(self.analyzer.clone()));
        let r: IndexReader = i.reader().unwrap();
        assert_eq!(r.searcher().num_docs(), self.reader.searcher().num_docs());
    }
}

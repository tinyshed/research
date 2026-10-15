use crate::{common::*, fixture, native};
use serde_json::json;
use std::time::Instant;
use tantivy::{
    Index, Term, collector::DocSetCollector, doc, query::TermQuery, schema::*,
    tokenizer::WhitespaceTokenizer,
};

fn size(path: &std::path::Path) -> u64 {
    std::fs::read_dir(path)
        .unwrap()
        .map(|v| {
            let p = v.unwrap().path();
            if p.is_dir() {
                size(&p)
            } else {
                bytes(p.to_str().unwrap())
            }
        })
        .sum()
}
pub fn run(a: &[String]) {
    let n: usize = a[0].parse().unwrap();
    let shape = &a[1];
    let c = open(&path());
    if !a.get(2).is_some_and(|s| s == "prepared") {
        fixture::seed(&c, n, shape, "ordered", 1000);
    }
    let dir = format!("{}.tantivy", path());
    std::fs::create_dir(&dir).unwrap();
    let mut schema = Schema::builder();
    let id = schema.add_u64_field("id", INDEXED | FAST);
    let text = schema.add_text_field(
        "terms",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("shared")
                .set_index_option(IndexRecordOption::Basic),
        ),
    );
    let schema = schema.build();
    let idle = memory();
    let cpu0 = cpu();
    let t = Instant::now();
    let index = Index::create_in_dir(&dir, schema).unwrap();
    index
        .tokenizers()
        .register("shared", WhitespaceTokenizer::default());
    let mut writer = index.writer_with_num_threads(1, 32_000_000).unwrap();
    let mut cursor = None;
    loop {
        let docs = fixture::read(&c, cursor, i64::MAX, 1000);
        if docs.is_empty() {
            break;
        }
        for d in &docs {
            writer
                .add_document(doc!(id=>d.id as u64,text=>d.terms.as_str()))
                .unwrap();
        }
        cursor = Some(docs.last().unwrap().id);
    }
    writer.commit().unwrap();
    let reader = index.reader().unwrap();
    let build_us = us(t);
    let resident = memory();
    let index_bytes = size(std::path::Path::new(&dir));
    let searcher = reader.searcher();
    let mut times = vec![];
    let mut checksum = 0;
    let qstart = Instant::now();
    for q in 0..300 {
        let t = Instant::now();
        let term = Term::from_field_text(text, &native::query_term(q, shape));
        let matches = searcher
            .search(
                &TermQuery::new(term, IndexRecordOption::Basic),
                &DocSetCollector,
            )
            .unwrap();
        let columns: Vec<_> = searcher
            .segment_readers()
            .iter()
            .map(|s| s.fast_fields().u64("id").unwrap())
            .collect();
        let mut ids: Vec<i64> = matches
            .iter()
            .map(|addr| {
                columns[addr.segment_ord as usize]
                    .first(addr.doc_id)
                    .unwrap() as i64
            })
            .collect();
        ids.sort_unstable();
        times.push(us(t));
        checksum ^= hash_ids(&ids);
    }
    let query_s = qstart.elapsed().as_secs_f64();
    let t = Instant::now();
    let fixture = fixture::Fixture::new(shape);
    for i in 1..=1000 {
        writer.delete_term(Term::from_field_u64(id, i));
        let d = fixture.doc(i as i64);
        writer
            .add_document(doc!(id=>i,text=>format!("{} revised",d.terms)))
            .unwrap();
    }
    writer.commit().unwrap();
    reader.reload().unwrap();
    let update_us = us(t);
    let workcpu = cpu() - cpu0;
    drop(writer);
    drop(searcher);
    drop(reader);
    drop(index);
    // Reopen proves durable index metadata, not cross-engine atomicity.
    let t = Instant::now();
    let reopened = Index::open_in_dir(&dir).unwrap();
    reopened
        .tokenizers()
        .register("shared", WhitespaceTokenizer::default());
    let r = reopened.reader().unwrap();
    assert_eq!(r.searcher().num_docs(), n as u64);
    let recovery_us = us(t);
    println!(
        "{}",
        json!({"experiment":"tantivy","rows":n,"shape":shape,"build_us":build_us,"build_rows_per_s":n as f64*1e6/build_us as f64,"queries":distribution(&mut times),"query_per_s":300./query_s,"query_checksum":checksum,"index_bytes":index_bytes,"final_index_bytes":size(std::path::Path::new(&dir)),"update_1000_commit_reload_us":update_us,"reopen_us":recovery_us,"idle_memory":idle,"resident_memory":resident,"final_memory":memory(),"cpu_s":workcpu,"allocations":allocations(),"tokenizer":"shared pre-normalized Russian Snowball + whitespace; term set, no ranking","atomicity":"independent commit; requires outbox/barrier or weaker semantics","status":"pass"})
    );
}

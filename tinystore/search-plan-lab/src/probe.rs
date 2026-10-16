//! What each piece adds to an executable: a program that indexes its
//! argument in FTS5 and searches it, built with one set of features at a
//! time. sizes.sh builds each set stripped and reports its bytes.
//!
//! | Features                   | What it links beyond SQLite with FTS5                   |
//! |----------------------------|---------------------------------------------------------|
//! | none                       | nothing: the baseline TinyStore already ships           |
//! | `analyzer`                 | Unicode words and NFKC, Snowball's stemmers             |
//! | `analyzer,typo`            | and the FST with its Levenshtein automata               |
//! | `analyzer,typo,icu`        | and ICU4X's word segmenter with its compiled data       |
//! | `analyzer,typo,tantivy`    | and Tantivy instead of FTS5's postings                  |

#[cfg(feature = "analyzer")]
mod analyze;
#[cfg(feature = "typo")]
mod typo;

fn main() {
    let text = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Кари́бский кризис café 世界和平".to_owned());
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute_batch("create virtual table idx using fts5(b, tokenize='ascii')")
        .unwrap();
    #[cfg(feature = "analyzer")]
    let text = {
        let analyzer = analyze::Analyzer::new(analyze::Analysis::named("auto"));
        let tokens = analyzer.tokens(&text);
        tokens
            .iter()
            .flat_map(|token| [Some(token.surface.clone()), token.stem.clone()])
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    };
    connection
        .execute("insert into idx(b) values (?1)", [&text])
        .unwrap();
    let first = text.split(' ').next().unwrap_or_default().to_owned();
    let found: i64 = connection
        .query_row(
            "select count(*) from idx where idx match ?1",
            [format!("\"{first}\"")],
            |row| row.get(0),
        )
        .unwrap();
    println!("{found}");
    #[cfg(feature = "typo")]
    {
        connection
            .execute_batch("create virtual table temp.vocab using fts5vocab(main, idx, 'row');")
            .unwrap();
        let typos = typo::Typos::build(&connection);
        println!(
            "{:?} of {} terms in {} bytes, {} ms",
            typos.candidates(&format!("{first}x")),
            typos.built.terms,
            typos.built.bytes,
            typos.built.millis
        );
    }
    #[cfg(feature = "tantivy")]
    {
        use tantivy::schema::{Schema, TEXT};
        let mut builder = Schema::builder();
        let body = builder.add_text_field("b", TEXT);
        let index = tantivy::Index::create_in_ram(builder.build());
        let mut writer = index
            .writer_with_num_threads::<tantivy::TantivyDocument>(1, 15 << 20)
            .unwrap();
        let mut document = tantivy::TantivyDocument::default();
        document.add_text(body, &text);
        writer.add_document(document).unwrap();
        writer.commit().unwrap();
        let searcher = index.reader().unwrap().searcher();
        let query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_text(body, &first),
            tantivy::schema::IndexRecordOption::WithFreqs,
        );
        println!(
            "{}",
            searcher.search(&query, &tantivy::collector::Count).unwrap()
        );
    }
}

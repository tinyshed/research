use serde_json::json;

use crate::Result;
use crate::pages::{self, Image};

pub(super) fn run() -> Result<()> {
    for pass in 1..=3 {
        let dir = tempfile::tempdir()?;
        let mut source = pages::writer(&dir.path().join("source.db"))?;
        source.execute_batch(
            "create table items(id integer primary key, value text);
            insert into items values(1, 'hello')",
        )?;
        let first = Image::read(&mut source)?;
        let path = dir.path().join("copy.db");
        first.apply(&path, first.pages.len() as u32)?;
        let copied = Image::read(&mut pages::reader(&path)?)?;
        println!(
            "{}",
            json!({"case":"initial_snapshot", "pass":pass,
            "sqlite":rusqlite::version(), "pages":first.pages.len(), "page_size":first.page_size,
            "raw_equal":first.pages==copied.pages, "canonical_equal":first.checksum()==copied.checksum()})
        );
        source.execute("insert into items values(2, 'world')", [])?;
        let next = Image::read(&mut source)?;
        let delta = Image {
            page_size: next.page_size,
            pages: next
                .pages
                .iter()
                .filter(|(n, p)| first.pages.get(n) != Some(p))
                .map(|(n, p)| (*n, p.clone()))
                .collect(),
        };
        delta.apply(&path, next.pages.len() as u32)?;
        let copied = Image::read(&mut pages::reader(&path)?)?;
        let mut differences = Vec::new();
        for (number, page) in &next.pages {
            for (offset, (a, b)) in page.iter().zip(&copied.pages[number]).enumerate() {
                if a != b {
                    differences.push(json!({"page":number,"offset":offset,"source":a,"copy":b}));
                }
            }
        }
        println!(
            "{}",
            json!({"case":"incremental_after_reopen", "pass":pass,
            "changed_pages":delta.pages.len(), "raw_equal":next.pages==copied.pages,
            "canonical_equal":next.checksum()==copied.checksum(), "differences":differences})
        );
    }
    Ok(())
}

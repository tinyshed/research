//! What a person kept, dropped and added, remembered for a store between
//! sessions in `.dashbin/see.toml`, as chapter 11 has it: never in the store
//! looked at. A panel is remembered by its key — the question's own title,
//! made of the schema's names — so a choice survives the schema changing
//! around it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::doc::{Doc, Panel};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Choices {
    #[serde(default)]
    pub kept: BTreeSet<String>,
    #[serde(default)]
    pub dropped: BTreeSet<String>,
    #[serde(default)]
    pub added: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    store: BTreeMap<String, Choices>,
}

pub struct State {
    path: PathBuf,
    file: File,
    store: String,
    undo: Vec<Choices>,
}

impl State {
    pub fn load(dir: &Path, store: &str) -> State {
        let path = dir.join("see.toml");
        let file = std::fs::read_to_string(&path).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default();
        State { path, file, store: store.to_string(), undo: Vec::new() }
    }

    pub fn choices(&self) -> Choices {
        self.file.store.get(&self.store).cloned().unwrap_or_default()
    }

    fn change(&mut self, f: impl FnOnce(&mut Choices)) {
        self.undo.push(self.choices());
        let c = self.file.store.entry(self.store.clone()).or_default();
        f(c);
        self.save();
    }

    pub fn keep(&mut self, key: &str) {
        let key = key.to_string();
        self.change(|c| {
            if !c.kept.remove(&key) {
                c.kept.insert(key.clone());
                c.dropped.remove(&key);
            }
        });
    }

    pub fn drop_panel(&mut self, key: &str) {
        let key = key.to_string();
        self.change(|c| {
            c.dropped.insert(key.clone());
            c.kept.remove(&key);
            c.added.retain(|k| k != &key);
        });
    }

    pub fn add(&mut self, key: &str) {
        let key = key.to_string();
        self.change(|c| {
            c.dropped.remove(&key);
            if !c.added.contains(&key) {
                c.added.push(key.clone());
            }
        });
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.file.store.insert(self.store.clone(), prev);
                self.save();
                true
            }
            None => false,
        }
    }

    fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = toml::to_string_pretty(&self.file) {
            let _ = std::fs::write(&self.path, s);
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The screen the choices make of a document: the five without the dropped,
/// each dropped one's place taken by the next of its role, then the added.
pub fn screen(doc: &Doc, c: &Choices) -> Vec<Panel> {
    let mut shown: Vec<Panel> = Vec::new();
    let taken = |shown: &Vec<Panel>, k: &str| shown.iter().any(|p| p.key == k);
    for p in &doc.panels {
        if !c.dropped.contains(&p.key) {
            shown.push(p.clone());
            continue;
        }
        let next = doc.spares.get(&p.role).and_then(|v| {
            v.iter().find(|s| !c.dropped.contains(&s.key) && !taken(&shown, &s.key) && !c.added.contains(&s.key))
        });
        if let Some(n) = next {
            shown.push(n.clone());
        }
    }
    for k in &c.added {
        if let Some(p) = spare(doc, k) {
            if !taken(&shown, k) {
                shown.push(p.clone());
            }
        }
    }
    shown
}

pub fn spare<'a>(doc: &'a Doc, key: &str) -> Option<&'a Panel> {
    doc.spares.values().flatten().chain(doc.panels.iter()).find(|p| p.key == key)
}

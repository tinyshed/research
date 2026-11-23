//! v2fly's domain lists as one table, a domain and the list it is written in, and two ways to hold
//! it. A line of `key<TAB>list`, the key `=name` when only that very name matches, is the text a
//! pack would carry.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub struct Compiled {
    pub text: String,
    pub lists: usize,
    pub suffix: usize,
    pub full: usize,
    pub includes: usize,
    pub regexp: usize,
    pub keyword: usize,
    /// The keys that more than one list writes itself; the first list by name keeps each.
    pub contested: usize,
}

/// Reads every list of `directory`. A list's `include:` is not followed: a domain is said to be of
/// the list that writes it, the narrowest one, and `google` including `youtube` adds nothing.
pub fn compile(directory: &Path) -> Compiled {
    let mut files: Vec<_> = std::fs::read_dir(directory).expect("lists").map(|entry| entry.expect("entry").path()).collect();
    files.sort();
    let mut keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let (mut includes, mut regexp, mut keyword) = (0, 0, 0);
    for file in &files {
        let list = file.file_name().expect("name").to_string_lossy().into_owned();
        for line in std::fs::read_to_string(file).expect("list").lines() {
            let rule = line.split('#').next().unwrap_or("").split_whitespace().next().unwrap_or("");
            let key = match rule.split_once(':') {
                _ if rule.is_empty() => continue,
                Some(("include", _)) => { includes += 1; continue }
                Some(("regexp", _)) => { regexp += 1; continue }
                Some(("keyword", _)) => { keyword += 1; continue }
                Some(("full", name)) => format!("={}", name.to_lowercase()),
                Some(("domain", name)) => name.to_lowercase(),
                Some(_) => continue,
                None => rule.to_lowercase(),
            };
            keys.entry(key).or_default().push(list.clone());
        }
    }
    let mut text = String::new();
    for (key, lists) in &keys {
        text.push_str(key);
        text.push('\t');
        text.push_str(&lists[0]);
        text.push('\n');
    }
    Compiled {
        lists: files.len(),
        suffix: keys.keys().filter(|key| !key.starts_with('=')).count(),
        full: keys.keys().filter(|key| key.starts_with('=')).count(),
        contested: keys.values().filter(|lists| lists.len() > 1).count(),
        text,
        includes,
        regexp,
        keyword,
    }
}

/// Names to look up as a store of traffic would hold them: four in five a host under a key of the
/// table, at a depth of none to three labels, the fifth a name no list has.
pub fn asked(text: &str, count: usize) -> Vec<String> {
    let keys: Vec<&str> = text.lines().filter_map(|line| line.split('\t').next()).collect();
    let under = ["", "www.", "rr4---sn-4g5e6nsz.", "a.b.c."];
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    (0..count)
        .map(|at| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if at % 5 == 4 {
                return format!("host{at}.nothing-{}.qqzz", state % 100_000);
            }
            let key = keys[(state % keys.len() as u64) as usize];
            match key.strip_prefix('=') {
                Some(name) => name.to_owned(),
                None => format!("{}{key}", under[(state >> 32) as usize % under.len()]),
            }
        })
        .collect()
}

/// The suffixes of `name` that start at a label, the whole name first.
fn suffixes(name: &str) -> impl Iterator<Item = &str> {
    std::iter::once(name).chain(name.match_indices('.').map(|(at, _)| &name[at + 1..]))
}

/// A hash map from a key to its list's number: one probe a suffix of the name asked.
pub struct Hashed {
    lists: Vec<Box<str>>,
    exact: HashMap<Box<str>, u16>,
    suffix: HashMap<Box<str>, u16>,
}

impl Hashed {
    pub fn read(text: &Path) -> Self {
        let text = std::fs::read_to_string(text).expect("text");
        let mut numbers: HashMap<&str, u16> = HashMap::new();
        let mut table = Self { lists: Vec::new(), exact: HashMap::new(), suffix: HashMap::new() };
        for line in text.lines() {
            let (key, list) = line.split_once('\t').expect("a key and a list");
            let number = *numbers.entry(list).or_insert_with(|| {
                table.lists.push(list.into());
                (table.lists.len() - 1) as u16
            });
            match key.strip_prefix('=') {
                Some(name) => table.exact.insert(name.into(), number),
                None => table.suffix.insert(key.into(), number),
            };
        }
        table
    }

    pub fn find(&self, name: &str) -> Option<&str> {
        let number = self.exact.get(name).or_else(|| suffixes(name).find_map(|suffix| self.suffix.get(suffix)))?;
        Some(&self.lists[*number as usize])
    }
}

/// The text itself, kept whole, and where each line starts: the lines are sorted by key already,
/// so a key is found by halving, and nothing is copied out of the file.
pub struct Sorted {
    text: String,
    lines: Vec<u32>,
}

impl Sorted {
    pub fn read(text: &Path) -> Self {
        let text = std::fs::read_to_string(text).expect("text");
        let mut lines = vec![0];
        lines.extend(text.match_indices('\n').map(|(at, _)| at as u32 + 1));
        lines.pop();
        Self { text, lines }
    }

    fn line(&self, at: usize) -> (&str, &str) {
        let rest = &self.text[self.lines[at] as usize..];
        let (key, rest) = rest.split_once('\t').expect("a key");
        (key, rest.split_once('\n').map_or(rest, |(list, _)| list))
    }

    fn get(&self, key: &str) -> Option<&str> {
        let at = self.lines.partition_point(|start| {
            let rest = &self.text[*start as usize..];
            &rest[..rest.find('\t').unwrap_or(rest.len())] < key
        });
        let (found, list) = (at < self.lines.len()).then(|| self.line(at))?;
        (found == key).then_some(list)
    }

    pub fn find(&self, name: &str) -> Option<&str> {
        let mut exact = String::with_capacity(name.len() + 1);
        exact.push('=');
        exact.push_str(name);
        self.get(&exact).or_else(|| suffixes(name).find_map(|suffix| self.get(suffix)))
    }
}

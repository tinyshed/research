//! The domains held as little as a hash can: every key in one text, where each starts, its list's
//! number, and a table of slots that says which key a hash leads to. No key is a string of its own
//! and no slot holds one.

use std::collections::HashMap;
use std::path::Path;

const EMPTY: u32 = u32::MAX;

pub struct Probed {
    keys: String,
    /// Where each key starts in `keys`, and after the last where they end.
    starts: Vec<u32>,
    list: Vec<u16>,
    /// A key's number at its hash, or at the first free slot after it.
    slots: Vec<u32>,
    lists: Vec<Box<str>>,
}

fn hashed(parts: [&str; 2]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in parts.iter().flat_map(|part| part.bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    hash ^ (hash >> 32)
}

impl Probed {
    pub fn read(text: &Path) -> Self {
        let text = std::fs::read_to_string(text).expect("text");
        let mut numbers: HashMap<&str, u16> = HashMap::new();
        let mut table = Self { keys: String::new(), starts: Vec::new(), list: Vec::new(), slots: Vec::new(), lists: Vec::new() };
        for line in text.lines() {
            let (key, list) = line.split_once('\t').expect("a key and a list");
            let number = *numbers.entry(list).or_insert_with(|| {
                table.lists.push(list.into());
                (table.lists.len() - 1) as u16
            });
            table.starts.push(table.keys.len() as u32);
            table.keys.push_str(key);
            table.list.push(number);
        }
        table.starts.push(table.keys.len() as u32);
        // three slots to two keys, rounded up to a power of two
        table.slots = vec![EMPTY; (table.list.len() * 3 / 2).next_power_of_two()];
        for at in 0..table.list.len() {
            let mut slot = hashed([table.key(at), ""]) as usize & (table.slots.len() - 1);
            while table.slots[slot] != EMPTY {
                slot = (slot + 1) & (table.slots.len() - 1);
            }
            table.slots[slot] = at as u32;
        }
        table.keys.shrink_to_fit();
        table.starts.shrink_to_fit();
        table.list.shrink_to_fit();
        table
    }

    fn key(&self, at: usize) -> &str {
        &self.keys[self.starts[at] as usize..self.starts[at + 1] as usize]
    }

    /// The list of the key that is `mark` and then `name`, the mark `=` for a name matched whole.
    fn get(&self, mark: &str, name: &str) -> Option<u16> {
        let mut slot = hashed([mark, name]) as usize & (self.slots.len() - 1);
        loop {
            let at = self.slots[slot];
            if at == EMPTY {
                return None;
            }
            if self.key(at as usize).strip_prefix(mark) == Some(name) && (mark.is_empty() != self.key(at as usize).starts_with('=')) {
                return Some(self.list[at as usize]);
            }
            slot = (slot + 1) & (self.slots.len() - 1);
        }
    }

    pub fn find(&self, name: &str) -> Option<&str> {
        let suffixes = std::iter::once(name).chain(name.match_indices('.').map(|(at, _)| &name[at + 1..]));
        let number = self.get("=", name).or_else(|| suffixes.into_iter().find_map(|suffix| self.get("", suffix)))?;
        Some(&self.lists[number as usize])
    }
}

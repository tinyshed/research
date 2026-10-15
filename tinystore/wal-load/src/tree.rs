use crate::pages::Pages;
use crate::{Result, require};
use sha2::{Digest, Sha256};

/// SHA-256 tree over canonical SQLite pages. Only the two pager-local
/// change counters are excluded, as established by the first research round.
#[derive(Clone, Default, Debug)]
pub(super) struct Tree {
    nodes: Vec<[u8; 32]>,
    pub batched: bool,
    pub leaves: u64,
    pub branches: u64,
    size: usize,
    pub count: u32,
    pub page_size: u32,
}

impl Tree {
    pub(super) fn apply(&mut self, pages: &Pages, count: u32, page_size: u32) -> Result<()> {
        require(
            count > 0 && (512..=65536).contains(&page_size) && page_size.is_power_of_two(),
            "invalid page geometry",
        )?;
        require(
            count as usize * page_size as usize <= crate::pages::MAX_IMAGE,
            "image limit exceeded",
        )?;
        if self.size < count as usize {
            self.grow((count as usize).next_power_of_two());
        }
        let mut dirty = Vec::new();
        for (number, page) in pages {
            require(
                *number > 0 && *number <= count && page.len() == page_size as usize,
                "invalid changed page",
            )?;
            self.leaves += 1;
            if self.batched {
                let index = self.size + *number as usize - 1;
                self.nodes[index] = leaf(*number, page);
                dirty.push(index / 2);
            } else {
                self.set(*number as usize - 1, leaf(*number, page));
            }
        }
        for number in count..self.count {
            if self.batched {
                let index = self.size + number as usize;
                self.nodes[index] = [0; 32];
                dirty.push(index / 2);
            } else {
                self.set(number as usize, [0; 32]);
            }
        }
        if self.batched {
            // Sorted changed leaves and a disjoint ascending truncation suffix.
            dirty.dedup();
            dirty.retain(|p| *p > 0);
            while !dirty.is_empty() {
                for index in &dirty {
                    self.branches += 1;
                    self.nodes[*index] = branch(self.nodes[*index * 2], self.nodes[*index * 2 + 1]);
                }
                for index in &mut dirty {
                    *index /= 2;
                }
                dirty.dedup();
                dirty.retain(|p| *p > 0);
            }
        }
        self.count = count;
        self.page_size = page_size;
        Ok(())
    }

    fn grow(&mut self, size: usize) {
        let mut nodes = vec![[0; 32]; 2 * size];
        if self.size > 0 {
            nodes[size..size + self.size].copy_from_slice(&self.nodes[self.size..2 * self.size]);
        }
        self.size = size;
        self.nodes = nodes;
        for index in (1..size).rev() {
            self.branches += 1;
            self.nodes[index] = branch(self.nodes[index * 2], self.nodes[index * 2 + 1]);
        }
    }

    fn set(&mut self, index: usize, hash: [u8; 32]) {
        let mut position = self.size + index;
        self.nodes[position] = hash;
        while position > 1 {
            position /= 2;
            self.branches += 1;
            self.nodes[position] = branch(self.nodes[position * 2], self.nodes[position * 2 + 1]);
        }
    }

    pub(super) fn bytes(&self) -> usize {
        self.nodes.capacity() * 32
    }
    pub(super) fn root(&self) -> [u8; 32] {
        if self.count == 0 {
            return [0; 32];
        }
        // Canonical width depends on the current file, not its past peak size.
        let width = (self.count as usize).next_power_of_two();
        let root = self.nodes[self.size / width];
        let mut hash = Sha256::new();
        hash.update(b"TSROOT02");
        hash.update(self.page_size.to_be_bytes());
        hash.update(self.count.to_be_bytes());
        hash.update(root);
        hash.finalize().into()
    }
}

fn branch(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update([1]);
    hash.update(a);
    hash.update(b);
    hash.finalize().into()
}

fn leaf(number: u32, page: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update([0]);
    hash.update(number.to_be_bytes());
    if number == 1 {
        hash.update(&page[..24]);
        hash.update([0; 4]);
        hash.update(&page[28..92]);
        hash.update([0; 4]);
        hash.update(&page[96..]);
    } else {
        hash.update(page);
    }
    hash.finalize().into()
}

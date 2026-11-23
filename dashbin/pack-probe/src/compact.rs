//! The same ranges held as little as they can be, and as bytes a hub would write once when a pack
//! is added and read back with no parsing.
//!
//! | What `Table` keeps a range      | What is kept here                                         |
//! |---------------------------------|-----------------------------------------------------------|
//! | its first and its last address  | its first: it ends where the next begins, a gap a range   |
//! | a v6 address whole, 16 bytes    | its upper half, 8: a range is whole /64s but a few        |
//! | its value's number in 4 bytes   | in as many bytes as the table's values need, 1 to 4       |
//! | each value a string of its own  | every value in one text, and where each ends              |

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;

/// The number of the value a gap between two ranges has.
const NONE: u32 = 0;

struct Packed {
    width: usize,
    bytes: Vec<u8>,
}

impl Packed {
    fn of(numbers: &[u32], width: usize) -> Self {
        Self { width, bytes: numbers.iter().flat_map(|number| number.to_le_bytes().into_iter().take(width)).collect() }
    }

    fn get(&self, at: usize) -> u32 {
        let mut whole = [0; 4];
        whole[..self.width].copy_from_slice(&self.bytes[at * self.width..(at + 1) * self.width]);
        u32::from_le_bytes(whole)
    }
}

struct Starts<A> {
    first: Vec<A>,
    value: Packed,
}

impl<A: Ord + Copy> Starts<A> {
    fn find(&self, address: A) -> u32 {
        match self.first.partition_point(|first| *first <= address).checked_sub(1) {
            Some(at) => self.value.get(at),
            None => NONE,
        }
    }
}

/// Ranges in order becoming starts: a gap before a range is a start of its own, and a range whose
/// value is the one before it is no start at all.
struct Building<A> {
    first: Vec<A>,
    value: Vec<u32>,
    /// Where the next range begins when it leaves no gap, none once the family's last address is passed.
    next: Option<A>,
}

impl<A: Ord + Copy> Building<A> {
    fn new(zero: A) -> Self {
        Self { first: Vec::new(), value: Vec::new(), next: Some(zero) }
    }

    fn start(&mut self, first: A, value: u32) {
        if self.value.last() != Some(&value) {
            self.first.push(first);
            self.value.push(value);
        }
    }

    fn range(&mut self, first: A, after: Option<A>, value: u32) {
        if let Some(next) = self.next.filter(|next| *next < first) {
            self.start(next, NONE);
        }
        self.start(first, value);
        self.next = after;
    }

    fn built(mut self, width: usize) -> Starts<A> {
        if let Some(next) = self.next {
            self.start(next, NONE);
        }
        self.first.shrink_to_fit();
        Starts { value: Packed::of(&self.value, width), first: self.first }
    }
}

pub struct Compact {
    v4: Starts<u32>,
    /// By the upper half of an address.
    v6: Starts<u64>,
    /// The v6 ranges that are not whole /64s, first, last and value: looked at before `v6`.
    odd: Vec<(u128, u128, u32)>,
    names: String,
    /// Where the value of each number ends in `names`; number 0 is no value and ends at 0.
    ends: Vec<u32>,
}

impl Compact {
    pub fn read(text: &Path) -> Self {
        let text = std::fs::read_to_string(text).expect("text");
        let mut numbers: HashMap<&str, u32> = HashMap::new();
        let (mut names, mut ends) = (String::new(), vec![0]);
        let rows: Vec<(IpAddr, IpAddr, u32)> = text
            .lines()
            .map(|line| {
                let mut fields = line.splitn(3, '\t');
                let (first, last, value) = (fields.next().expect("first"), fields.next().expect("last"), fields.next().expect("value"));
                let number = *numbers.entry(value).or_insert_with(|| {
                    names.push_str(value);
                    ends.push(names.len() as u32);
                    ends.len() as u32 - 1
                });
                (first.parse().expect("an address"), last.parse().expect("an address"), number)
            })
            .collect();
        let width = ends.len().next_power_of_two().trailing_zeros().div_ceil(8).max(1) as usize;
        let (mut v4, mut v6, mut odd) = (Building::new(0_u32), Building::new(0_u64), Vec::new());
        for (first, last, number) in rows {
            match (first, last) {
                (IpAddr::V4(first), IpAddr::V4(last)) => v4.range(first.to_bits(), last.to_bits().checked_add(1), number),
                (IpAddr::V6(first), IpAddr::V6(last)) => {
                    let (first, last) = (first.to_bits(), last.to_bits());
                    if first as u64 == 0 && last as u64 == u64::MAX {
                        v6.range((first >> 64) as u64, ((last >> 64) as u64).checked_add(1), number);
                    } else {
                        odd.push((first, last, number));
                    }
                }
                _ => unreachable!("a range of one family"),
            }
        }
        names.shrink_to_fit();
        ends.shrink_to_fit();
        odd.shrink_to_fit();
        Self { v4: v4.built(width), v6: v6.built(width), odd, names, ends }
    }

    pub fn find(&self, address: IpAddr) -> Option<&str> {
        let number = match address {
            IpAddr::V4(address) => self.v4.find(address.to_bits()),
            IpAddr::V6(address) => {
                let address = address.to_bits();
                let at = self.odd.partition_point(|(first, _, _)| *first <= address).checked_sub(1);
                match at.map(|at| self.odd[at]).filter(|(_, last, _)| address <= *last) {
                    Some((_, _, number)) => number,
                    None => self.v6.find((address >> 64) as u64),
                }
            }
        };
        (number != NONE).then(|| &self.names[self.ends[number as usize - 1] as usize..self.ends[number as usize] as usize])
    }

    /// The table as bytes: each column behind its length, numbers little-endian.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut put = |bytes: Vec<u8>| {
            out.extend((bytes.len() as u64).to_le_bytes());
            out.extend(bytes);
        };
        put(vec![self.v4.value.width as u8]);
        put(self.v4.first.iter().flat_map(|one| one.to_le_bytes()).collect());
        put(self.v4.value.bytes.clone());
        put(self.v6.first.iter().flat_map(|one| one.to_le_bytes()).collect());
        put(self.v6.value.bytes.clone());
        put(self.odd.iter().flat_map(|(first, last, number)| [&first.to_le_bytes()[..], &last.to_le_bytes(), &number.to_le_bytes()].concat()).collect());
        put(self.names.clone().into_bytes());
        put(self.ends.iter().flat_map(|one| one.to_le_bytes()).collect());
        out
    }

    pub fn load(file: &Path) -> Self {
        Self::of(&std::fs::read(file).expect("bytes"))
    }

    pub fn of(bytes: &[u8]) -> Self {
        let mut rest = bytes;
        let mut take = || {
            let (length, after) = rest.split_at(8);
            let (taken, after) = after.split_at(u64::from_le_bytes(length.try_into().expect("a length")) as usize);
            rest = after;
            taken
        };
        let width = take()[0] as usize;
        let v4 = Starts { first: numbers(take(), u32::from_le_bytes), value: Packed { width, bytes: take().to_vec() } };
        let v6 = Starts { first: numbers(take(), u64::from_le_bytes), value: Packed { width, bytes: take().to_vec() } };
        let odd = take()
            .chunks_exact(36)
            .map(|one| {
                let part = |from: usize| u128::from_le_bytes(one[from..from + 16].try_into().expect("an address"));
                (part(0), part(16), u32::from_le_bytes(one[32..].try_into().expect("a number")))
            })
            .collect();
        let names = String::from_utf8(take().to_vec()).expect("names");
        Self { v4, v6, odd, names, ends: numbers(take(), u32::from_le_bytes) }
    }
}

fn numbers<const N: usize, A>(bytes: &[u8], of: fn([u8; N]) -> A) -> Vec<A> {
    bytes.chunks_exact(N).map(|one| of(one.try_into().expect("a number's bytes"))).collect()
}

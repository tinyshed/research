//! DB-IP's ranges as one table, `first<TAB>last<TAB>value`, and the sorted columns a hub would
//! hold it in: an address is found by halving among the ranges' first addresses.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;

pub struct Compiled {
    pub text: String,
    pub v4: usize,
    pub v6: usize,
    /// The v6 ranges that begin and end on a /64, which half the bytes would hold.
    pub v6_whole: usize,
    pub values: usize,
    /// Each range begins after the one before it ends, v4 before v6.
    pub ordered: bool,
}

/// A line of DB-IP's CSV: `first,last,CC`, or `first,last,number,"name"` for a network's owner,
/// which is written here as `AS13335 Cloudflare, Inc.`.
pub fn compile(csv: &Path) -> Compiled {
    let mut compiled = Compiled { text: String::new(), v4: 0, v6: 0, v6_whole: 0, values: 0, ordered: true };
    let mut values: HashMap<String, ()> = HashMap::new();
    let mut before: Option<IpAddr> = None;
    for line in std::fs::read_to_string(csv).expect("csv").lines() {
        let mut fields = line.splitn(4, ',');
        let (first, last) = (fields.next().expect("first"), fields.next().expect("last"));
        let value = match (fields.next().expect("value"), fields.next()) {
            (code, None) => code.to_owned(),
            (number, Some(name)) => format!("AS{number} {}", name.trim_matches('"').replace("\"\"", "\"")),
        };
        let (from, to): (IpAddr, IpAddr) = (first.parse().expect("an address"), last.parse().expect("an address"));
        compiled.ordered &= before.is_none_or(|end| end < from) && from <= to;
        before = Some(to);
        match (from, to) {
            (IpAddr::V4(_), _) => compiled.v4 += 1,
            (IpAddr::V6(from), IpAddr::V6(to)) => {
                compiled.v6 += 1;
                let whole = from.to_bits() as u64 == 0 && to.to_bits() as u64 == u64::MAX;
                compiled.v6_whole += usize::from(whole);
            }
            _ => unreachable!("a range of one family"),
        }
        values.insert(value.clone(), ());
        compiled.text.push_str(&format!("{first}\t{last}\t{value}\n"));
    }
    compiled.values = values.len();
    compiled
}

/// Addresses to look up: half v4 and half v6 of the part of the space that is given out.
pub fn asked(count: usize) -> Vec<IpAddr> {
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    (0..count)
        .map(|at| {
            if at % 2 == 0 {
                IpAddr::V4((next() as u32).into())
            } else {
                // 2000::/3 down to 2c0f::, where the registries' v6 is
                let high = 0x2000_0000_0000_0000 | (next() % 0x0c10_0000_0000_0000);
                IpAddr::V6(((u128::from(high) << 64) | u128::from(next())).into())
            }
        })
        .collect()
}

struct Family<A> {
    first: Vec<A>,
    last: Vec<A>,
    value: Vec<u32>,
}

impl<A: Ord + Copy> Family<A> {
    fn new() -> Self {
        Self { first: Vec::new(), last: Vec::new(), value: Vec::new() }
    }

    fn find(&self, address: A) -> Option<u32> {
        let at = self.first.partition_point(|first| *first <= address).checked_sub(1)?;
        (address <= self.last[at]).then(|| self.value[at])
    }

    fn fit(&mut self) {
        self.first.shrink_to_fit();
        self.last.shrink_to_fit();
        self.value.shrink_to_fit();
    }
}

pub struct Table {
    v4: Family<u32>,
    v6: Family<u128>,
    values: Vec<Box<str>>,
}

impl Table {
    pub fn read(text: &Path) -> Self {
        let text = std::fs::read_to_string(text).expect("text");
        let mut numbers: HashMap<&str, u32> = HashMap::new();
        let mut table = Self { v4: Family::new(), v6: Family::new(), values: Vec::new() };
        for line in text.lines() {
            let mut fields = line.splitn(3, '\t');
            let (first, last, value) = (fields.next().expect("first"), fields.next().expect("last"), fields.next().expect("value"));
            let number = *numbers.entry(value).or_insert_with(|| {
                table.values.push(value.into());
                (table.values.len() - 1) as u32
            });
            match (first.parse().expect("an address"), last.parse().expect("an address")) {
                (IpAddr::V4(first), IpAddr::V4(last)) => {
                    table.v4.first.push(first.to_bits());
                    table.v4.last.push(last.to_bits());
                    table.v4.value.push(number);
                }
                (IpAddr::V6(first), IpAddr::V6(last)) => {
                    table.v6.first.push(first.to_bits());
                    table.v6.last.push(last.to_bits());
                    table.v6.value.push(number);
                }
                _ => unreachable!("a range of one family"),
            }
        }
        table.v4.fit();
        table.v6.fit();
        table
    }

    pub fn find(&self, address: IpAddr) -> Option<&str> {
        let number = match address {
            IpAddr::V4(address) => self.v4.find(address.to_bits()),
            IpAddr::V6(address) => self.v6.find(address.to_bits()),
        }?;
        Some(&self.values[number as usize])
    }
}

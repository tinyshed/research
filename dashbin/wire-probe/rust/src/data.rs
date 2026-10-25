// the two datasets: "mixed" holds every type with nulls and bounds at random, "series" is what a chart reads
use crate::{Column, Data, Frame};

fn column(name: &str, unit: Option<&str>, data: Data, nulls: Option<Vec<bool>>) -> Column {
    Column { name: name.into(), unit: unit.map(Into::into), data, nulls }
}

const START_US: i64 = 1_790_812_800_000_000; // 2026-10-01T00:00:00Z
const SITES: [&str; 20] = [
    "kazan", "spb", "msk", "nsk", "ekb", "nn", "samara", "omsk", "rostov", "ufa", "perm", "voronezh", "volgograd",
    "krasnodar", "saratov", "tyumen", "izhevsk", "barnaul", "irkutsk", "vladivostok",
];
const WORDS: [&str; 16] = [
    "payment", "failed", "order", "shipped", "refund", "заказ", "оплата", "возврат", "доставка", "клиент", "✅", "🚀",
    "timeout", "retry", "ok", "ошибка",
];

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn nulls(rng: &mut Rng, rows: usize, percent: u64) -> Option<Vec<bool>> {
    Some((0..rows).map(|_| rng.below(100) < percent).collect())
}

fn texts(values: impl Iterator<Item = String>) -> Data {
    let mut offsets = vec![0u32];
    let mut bytes = Vec::new();
    for v in values {
        bytes.extend_from_slice(v.as_bytes());
        offsets.push(bytes.len() as u32);
    }
    Data::Text { offsets, bytes }
}

fn times(rng: &mut Rng, rows: usize) -> Vec<i64> {
    let mut t = START_US;
    (0..rows)
        .map(|_| {
            t += 15_000_000 + rng.below(1_000_000) as i64;
            t
        })
        .collect()
}

fn amounts(rng: &mut Rng, rows: usize) -> Vec<i128> {
    let edge: i128 = 10i128.pow(38) - 1;
    (0..rows)
        .map(|i| match i % 5000 {
            1 => edge,
            2 => -edge,
            _ => rng.below(1_000_000_000) as i128 - 500_000_000,
        })
        .collect()
}

pub fn mixed(rows: usize) -> Frame {
    let mut rng = Rng::new(0x5eed_d45b_1a00_0001);
    let time = times(&mut rng, rows);
    let wall: Vec<i64> = time.iter().map(|t| t + 3 * 3_600_000_000).collect();
    let day: Vec<i32> = (0..rows).map(|_| -7300 + rng.below(21_900) as i32).collect();
    let count: Vec<i64> = (0..rows).map(|_| rng.below(10_000) as i64).collect();
    let big: Vec<i64> = (0..rows)
        .map(|i| match i % 1000 {
            1 => i64::MIN,
            2 => i64::MAX,
            3 => 9_007_199_254_740_993,
            4 => -9_007_199_254_740_993,
            _ => rng.next() as i64,
        })
        .collect();
    let ratio: Vec<f64> = (0..rows)
        .map(|i| {
            if i % 997 == 5 {
                f64::NAN
            } else if i % 1999 == 7 {
                f64::INFINITY
            } else {
                rng.unit()
            }
        })
        .collect();
    let amount = amounts(&mut rng, rows);
    let site = texts((0..rows).map(|_| SITES[rng.below(20) as usize].to_string()));
    let message = texts((0..rows).map(|i| {
        let words = if i % 1000 == 9 { 600 } else { rng.below(12) as usize };
        (0..words).map(|_| WORDS[rng.below(16) as usize]).collect::<Vec<_>>().join(" ")
    }));
    let ok: Vec<bool> = (0..rows).map(|_| rng.below(2) == 0).collect();
    Frame {
        rows,
        columns: vec![
            column("time", None, Data::Time(time), None),
            column("wall", None, Data::Wall(wall), nulls(&mut rng, rows, 5)),
            column("day", None, Data::Date(day), nulls(&mut rng, rows, 5)),
            column("count", None, Data::Int(count), nulls(&mut rng, rows, 5)),
            column("big", None, Data::Int(big), nulls(&mut rng, rows, 5)),
            column("ratio", Some("ratio"), Data::Float(ratio), nulls(&mut rng, rows, 5)),
            column("amount", Some("EUR"), Data::Decimal { mantissa: amount, scale: 2 }, nulls(&mut rng, rows, 5)),
            column("site", None, site, None),
            column("message", None, message, nulls(&mut rng, rows, 10)),
            column("ok", None, Data::Bool(ok), nulls(&mut rng, rows, 5)),
        ],
    }
}

// twenty sites scraped every 15 s, the shape a chart over a month of metrics actually reads
pub fn series(rows: usize) -> Frame {
    let mut rng = Rng::new(0x5eed_d45b_1a00_0002);
    let sites = SITES.len();
    let time: Vec<i64> = (0..rows).map(|i| START_US + (i / sites) as i64 * 15_000_000).collect();
    let wave = |i: usize| (std::f64::consts::TAU * (i / sites) as f64 / 5760.0).sin();
    let count: Vec<i64> = (0..rows).map(|i| (500.0 + 400.0 * wave(i)) as i64 + rng.below(50) as i64).collect();
    let ratio: Vec<f64> = (0..rows).map(|i| ((0.5 + 0.3 * wave(i) + rng.unit() * 0.1) * 1e4).round() / 1e4).collect();
    let amount: Vec<i128> = (0..rows).map(|_| 100 + rng.below(100_000) as i128).collect();
    let site = texts((0..rows).map(|i| SITES[i % sites].to_string()));
    Frame {
        rows,
        columns: vec![
            column("time", None, Data::Time(time), None),
            column("site", None, site, None),
            column("count", None, Data::Int(count), nulls(&mut rng, rows, 2)),
            column("ratio", Some("ratio"), Data::Float(ratio), nulls(&mut rng, rows, 2)),
            column("amount", Some("EUR"), Data::Decimal { mantissa: amount, scale: 2 }, nulls(&mut rng, rows, 2)),
        ],
    }
}

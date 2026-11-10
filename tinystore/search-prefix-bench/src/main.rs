//! The heap a TinyStore search holds with `prefix` and with `prefix_every`
//! above the same search without, against the store's memory budget, on
//! indexes generated from a seed (reports/search-prefix-memory-2026-11-10.md).
//!
//! A document is words drawn by Zipf's law of exponent 1 from a vocabulary of
//! 50,000 words, each of five to nine letters of an alphabet of eight, the
//! ranks given in the order the words were made: two letters begin some 780
//! words and common ones among them, as `th` begins `the` and `that`.
//!
//! ```text
//! search-prefix-bench build <dir> <documents> <words a document> <seed>
//! search-prefix-bench measure <dir> <memory: none | bytes> <heap passes> <time passes>
//! ```
//!
//! `build` writes the store into `<dir>/store`, the vocabulary with how many
//! documents hold each word into `<dir>/words.tsv`, and a JSON line. `measure`
//! prints a JSON line a search: the heap it held at most above what was held
//! before it, by this process's counting allocator; the store's memory it
//! reserved at most, read by a thread beside it; SQLite's own peak, by
//! SQLite's counter, since SQLite allocates through its own malloc and the
//! allocator does not see it; what it answered; and, in passes of their own
//! without the thread beside, its time.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use serde_json::{Value, json};
use tinystore::search::Index;
use tinystore::{Durability, Options, Store};

/// The system's allocator, counting the bytes it holds and the most it held,
/// as TinyStore's `tests/metrics_memory.rs` counts them: unless
/// `PREFIX_REALLOC=in-place`, `realloc` is the trait's own, a new block, a
/// copy and the old one freed, both counted at once; with it, the system's
/// `realloc`, counted by what it adds, as a block grown where it lies holds.
/// The first bounds a growth's moment from above, the second from below.
struct Counting;

static HELD: AtomicU64 = AtomicU64::new(0);
static MOST: AtomicU64 = AtomicU64::new(0);
static IN_PLACE: AtomicBool = AtomicBool::new(false);

fn add(bytes: usize) {
    let held = HELD.fetch_add(bytes as u64, Ordering::Relaxed) + bytes as u64;
    MOST.fetch_max(held, Ordering::Relaxed);
}

// SAFETY: every call is the system allocator's, with the caller's own
// arguments; the counters beside it only add and take away sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        add(layout.size());
        // SAFETY: the layout is the caller's, as `alloc`'s contract asks.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HELD.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: the pointer came from `alloc` above with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if !IN_PLACE.load(Ordering::Relaxed) {
            // SAFETY: `realloc`'s contract makes `new_size`, rounded up to
            // `layout.align()`, a valid size for that alignment.
            let grown = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
            // SAFETY: as `alloc` above; then the two blocks do not overlap, each
            // holds the bytes copied, and `ptr` came from this allocator.
            unsafe {
                let new = self.alloc(grown);
                if !new.is_null() {
                    std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                    self.dealloc(ptr, layout);
                }
                return new;
            }
        }
        // SAFETY: the caller's arguments, as `realloc`'s contract asks.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            if new_size >= layout.size() {
                add(new_size - layout.size());
            } else {
                HELD.fetch_sub((layout.size() - new_size) as u64, Ordering::Relaxed);
            }
        }
        new
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

const ALPHABET: &[u8] = b"abcdefgh";
const VOCABULARY: usize = 50_000;
const SHORTEST: u64 = 5;
const LONGEST: u64 = 9;
const INDEX: &str = "docs";
const BATCH: usize = 1000;
/// How `search/read.rs` reads a prefix at TinyStore `02231bd0`: the 64 words
/// most documents hold of those it begins, in a field (`MOST_PREFIXED`), and
/// with `prefix_every` the first eight words (`MOST_PREFIXES`) of two letters
/// or more (`LEAST_PREFIX`).
const MOST_PREFIXED: usize = 64;
const MOST_PREFIXES: usize = 8;
const LEAST_PREFIX: usize = 2;
/// The keys' parts of words as written are numbered below this, their stems' from it.
const STEMS: i64 = 1 << 30;

fn main() {
    IN_PLACE.store(std::env::var("PREFIX_REALLOC").is_ok_and(|how| how == "in-place"), Ordering::SeqCst);
    let args: Vec<String> = std::env::args().collect();
    match (args.get(1).map(String::as_str), args.len()) {
        (Some("build"), 6) => build(Path::new(&args[2]), number(&args[3]), number(&args[4]), number(&args[5])),
        (Some("measure"), 6) => {
            let memory = (args[3] != "none").then(|| number(&args[3]));
            measure(Path::new(&args[2]), memory, number(&args[4]), number(&args[5]));
        }
        _ => {
            eprintln!("search-prefix-bench build <dir> <documents> <words a document> <seed>");
            eprintln!("search-prefix-bench measure <dir> <memory: none | bytes> <heap passes> <time passes>");
            std::process::exit(2);
        }
    }
}

fn number<T: std::str::FromStr>(text: &str) -> T {
    text.parse().unwrap_or_else(|_| panic!("not a number: {text}"))
}

/// splitmix64: a stream of numbers from a seed, the same on every machine.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// A number in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// The vocabulary by rank, the most frequent first.
fn vocabulary(seed: u64) -> Vec<String> {
    let mut random = Random(seed ^ 0x766f_6361_6275_6c61);
    let mut seen = HashSet::new();
    let mut words = Vec::with_capacity(VOCABULARY);
    while words.len() < VOCABULARY {
        let letters = SHORTEST + random.below(LONGEST - SHORTEST + 1);
        let word: String =
            (0..letters).map(|_| char::from(ALPHABET[random.below(ALPHABET.len() as u64) as usize])).collect();
        if seen.insert(word.clone()) {
            words.push(word);
        }
    }
    words
}

/// The documents' words by rank, drawn by Zipf's law of exponent 1.
struct Documents {
    random: Random,
    cumulative: Vec<f64>,
    words: usize,
}

impl Documents {
    fn new(seed: u64, words: usize) -> Documents {
        let mut sum = 0.0;
        let mut cumulative: Vec<f64> = (1..=VOCABULARY)
            .map(|rank| {
                sum += 1.0 / rank as f64;
                sum
            })
            .collect();
        for share in &mut cumulative {
            *share /= sum;
        }
        Documents { random: Random(seed ^ 0x646f_6375_6d65_6e74), cumulative, words }
    }

    fn next(&mut self, ranks: &mut Vec<usize>) {
        ranks.clear();
        for _ in 0..self.words {
            let drawn = self.random.unit();
            ranks.push(self.cumulative.partition_point(|share| *share < drawn).min(VOCABULARY - 1));
        }
    }
}

/// Sets every document, a thousand a commit and two sets in flight while
/// background work merges, as search-msmarco's harness sets, then merges what
/// is left into one segment, as `search::maintain` does.
fn build(dir: &Path, documents: usize, words: usize, seed: u64) {
    let started = Instant::now();
    let vocabulary = vocabulary(seed);
    let mut drawn = Documents::new(seed, words);
    let mut holding = vec![0_u64; VOCABULARY];
    let mut last = vec![usize::MAX; VOCABULARY];
    let store_dir = dir.join("store");
    fs::create_dir_all(&store_dir).unwrap();
    let options = Options { background: true, durability: Some(Durability::Os), ..Options::default() };
    let store = Store::open(&store_dir, options).unwrap();
    let index: Index<String> = store.index::<String>(INDEX).open().unwrap();
    let (send, receive) = mpsc::sync_channel::<Vec<(String, String)>>(2);
    let receive = Mutex::new(receive);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                loop {
                    let next = receive.lock().unwrap().recv();
                    let Ok(batch) = next else { return };
                    index.set_many(batch.iter().map(|(id, text)| (id.as_str(), text))).unwrap();
                }
            });
        }
        let mut ranks = Vec::with_capacity(words);
        let mut batch = Vec::with_capacity(BATCH);
        for doc in 0..documents {
            drawn.next(&mut ranks);
            let mut text = String::with_capacity(words * 10);
            for (at, &rank) in ranks.iter().enumerate() {
                if last[rank] != doc {
                    last[rank] = doc;
                    holding[rank] += 1;
                }
                if at > 0 {
                    text.push(' ');
                }
                text.push_str(&vocabulary[rank]);
            }
            batch.push((doc.to_string(), text));
            if batch.len() == BATCH {
                send.send(std::mem::replace(&mut batch, Vec::with_capacity(BATCH))).unwrap();
            }
            if (doc + 1) % 500_000 == 0 {
                eprintln!("{} documents, {:.0} s", doc + 1, started.elapsed().as_secs_f64());
            }
        }
        if !batch.is_empty() {
            send.send(batch).unwrap();
        }
        drop(send);
    });
    let set_seconds = started.elapsed().as_secs_f64();
    tinystore::search::maintain(&store).unwrap();
    let seconds = started.elapsed().as_secs_f64();
    drop(index);
    store.close().unwrap();
    drop(store);
    let mut tsv = String::new();
    for (rank, word) in vocabulary.iter().enumerate() {
        tsv += &format!("{rank}\t{word}\t{}\n", holding[rank]);
    }
    fs::write(dir.join("words.tsv"), tsv).unwrap();
    let search_db = store_dir.join("search.db");
    let line = json!({
        "step": "build", "index": format!("{documents}x{words}"), "documents": documents, "words": words,
        "seed": seed, "vocabulary": VOCABULARY, "alphabet": String::from_utf8_lossy(ALPHABET),
        "postings": holding.iter().sum::<u64>(), "set_seconds": set_seconds, "seconds": seconds,
        "search_db_bytes": fs::metadata(&search_db).map(|meta| meta.len()).unwrap_or(0),
        "keys": keys(&search_db, &vocabulary, &holding),
    });
    fs::write(dir.join("fixture.json"), line.to_string()).unwrap();
    println!("{line}");
}

/// The keys' rows of the words as written against what the fixture counted:
/// how many rows count a word's documents otherwise, and each part's keys,
/// since `FIND_PREFIXED` chooses a prefix's words by these rows.
fn keys(file: &Path, vocabulary: &[String], holding: &[u64]) -> Value {
    let connection = rusqlite::Connection::open_with_flags(file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut select = connection.prepare("select term, documents from _tinystore_search_keys where part < ?1").unwrap();
    let rows: HashMap<String, i64> =
        select.query_map([STEMS], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().map(Result::unwrap).collect();
    let otherwise = vocabulary
        .iter()
        .zip(holding)
        .filter(|(word, held)| rows.get(word.as_str()).copied().unwrap_or(0) != **held as i64)
        .count();
    let mut parts = connection.prepare("select part, count(*), sum(documents) from _tinystore_search_keys group by part").unwrap();
    let parts: Vec<Value> = parts
        .query_map([], |row| {
            Ok(json!({"part": row.get::<_, i64>(0)?, "keys": row.get::<_, i64>(1)?, "documents": row.get::<_, i64>(2)?}))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    json!({"written": rows.len(), "counted_otherwise": otherwise, "parts": parts})
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Whole,
    Last,
    Every,
}

/// A prefix and the words of the index it is read as, as `FIND_PREFIXED`
/// chooses them: of the words it begins, the 64 most documents hold, ties by
/// their text; and the postings those words hold.
#[derive(Clone)]
struct Chosen {
    prefix: String,
    begins: usize,
    words: Vec<String>,
    postings: u64,
}

struct Arm {
    name: String,
    text: String,
    mode: Mode,
    prefixes: Vec<Chosen>,
}

fn read_words(dir: &Path) -> Vec<(String, u64)> {
    let tsv = fs::read_to_string(dir.join("words.tsv")).unwrap();
    tsv.lines()
        .map(|line| {
            let mut cells = line.split('\t').skip(1);
            (cells.next().unwrap().to_owned(), number(cells.next().unwrap()))
        })
        .collect()
}

/// Every prefix of `letters` letters the index's words begin, chosen as the
/// engine chooses, the one whose words hold the most postings first.
fn prefixes(words: &[(String, u64)], letters: usize) -> Vec<Chosen> {
    let mut begun: HashMap<&str, Vec<&(String, u64)>> = HashMap::new();
    for word in words.iter().filter(|(text, held)| *held > 0 && text.len() > letters) {
        begun.entry(&word.0[..letters]).or_default().push(word);
    }
    let mut all: Vec<Chosen> = begun
        .into_iter()
        .map(|(prefix, mut words)| {
            let begins = words.len();
            words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            words.truncate(MOST_PREFIXED);
            let postings = words.iter().map(|word| word.1).sum();
            Chosen { prefix: prefix.to_owned(), begins, words: words.iter().map(|word| word.0.clone()).collect(), postings }
        })
        .collect();
    all.sort_by(|a, b| b.postings.cmp(&a.postings).then(a.prefix.cmp(&b.prefix)));
    all
}

/// The least, the median, the 90th percentile and the most of the postings
/// the prefixes' words hold.
fn spread(all: &[Chosen]) -> Value {
    let mut postings: Vec<u64> = all.iter().map(|chosen| chosen.postings).collect();
    postings.sort_unstable();
    let at = |share: f64| postings[((postings.len() - 1) as f64 * share).round() as usize];
    json!({"prefixes": postings.len(), "least": at(0.0), "median": at(0.5), "p90": at(0.9), "most": at(1.0)})
}

/// The searches, each beside the same text read without a prefix: the most
/// common word and the prefix of `letters` letters whose words hold the most
/// postings, read with `prefix`; the eight such prefixes, read with
/// `prefix_every`; and the eight most common words, whole.
fn arms(words: &[(String, u64)]) -> (Vec<Arm>, Value) {
    let first = &words[0].0;
    let mut arms = Vec::new();
    let mut spreads = Vec::new();
    for letters in [1, 2, 4] {
        let all = prefixes(words, letters);
        spreads.push(json!({"letters": letters, "read_as_prefix": letters >= LEAST_PREFIX, "postings": spread(&all)}));
        let worst = vec![all[0].clone()];
        let text = format!("{first} {}", all[0].prefix);
        arms.push(Arm { name: format!("last {letters}, off"), text: text.clone(), mode: Mode::Whole, prefixes: worst.clone() });
        arms.push(Arm { name: format!("last {letters}"), text, mode: Mode::Last, prefixes: worst });
        let eight: Vec<Chosen> = all.iter().take(MOST_PREFIXES).cloned().collect();
        let text = eight.iter().map(|chosen| chosen.prefix.as_str()).collect::<Vec<_>>().join(" ");
        arms.push(Arm { name: format!("every {letters}, off"), text: text.clone(), mode: Mode::Whole, prefixes: eight.clone() });
        arms.push(Arm { name: format!("every {letters}"), text, mode: Mode::Every, prefixes: eight });
    }
    let common: Vec<&str> = words.iter().take(8).map(|word| word.0.as_str()).collect();
    arms.push(Arm { name: "8 common".into(), text: common.join(" "), mode: Mode::Whole, prefixes: Vec::new() });
    (arms, json!(spreads))
}

fn search(index: &Index<String>, arm: &Arm) -> tinystore::Result<usize> {
    let call = index.search(&arm.text);
    let call = match arm.mode {
        Mode::Whole => call,
        Mode::Last => call.prefix(),
        Mode::Every => call.prefix_every(),
    };
    call.run().map(|found| found.hits.len())
}

/// What a call held: the most heap above what was held before it, what it
/// left held, the most of the store's memory it reserved, and SQLite's most
/// above what SQLite held before it.
struct Held {
    peak: u64,
    retained: i64,
    reserved: u64,
    sqlite: i64,
}

/// `call`, and what it held, the store's memory read by a thread beside it as
/// TinyStore's `metrics_memory` test reads it.
fn measured<R>(store: &Store, call: impl FnOnce() -> R) -> (R, Held) {
    let memory = Arc::clone(store.memory());
    let reserved_before = memory.used();
    let (done, started) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let reserved = Arc::new(AtomicU64::new(reserved_before));
    let watching = {
        let (done, started, reserved) = (Arc::clone(&done), Arc::clone(&started), Arc::clone(&reserved));
        std::thread::spawn(move || {
            started.store(true, Ordering::SeqCst);
            while !done.load(Ordering::SeqCst) {
                reserved.fetch_max(memory.used(), Ordering::SeqCst);
                std::hint::spin_loop();
            }
        })
    };
    while !started.load(Ordering::SeqCst) {
        std::hint::spin_loop();
    }
    // SAFETY: both read or reset counters SQLite keeps under its own mutex,
    // from any thread, and take no pointer.
    let sqlite_before = unsafe { rusqlite::ffi::sqlite3_memory_used() };
    // SAFETY: as above.
    unsafe { rusqlite::ffi::sqlite3_memory_highwater(1) };
    let before = HELD.load(Ordering::SeqCst);
    MOST.store(before, Ordering::SeqCst);
    let answer = call();
    let (most, after) = (MOST.load(Ordering::SeqCst), HELD.load(Ordering::SeqCst));
    // SAFETY: as above.
    let sqlite_most = unsafe { rusqlite::ffi::sqlite3_memory_highwater(0) };
    done.store(true, Ordering::SeqCst);
    watching.join().unwrap();
    let held = Held {
        peak: most - before,
        retained: after as i64 - before as i64,
        reserved: reserved.load(Ordering::SeqCst).saturating_sub(reserved_before),
        sqlite: sqlite_most - sqlite_before,
    };
    (answer, held)
}

/// The arms in their order on even passes and the other way on odd ones.
fn ordered(arms: &[Arm], pass: usize) -> Vec<&Arm> {
    let mut order: Vec<&Arm> = arms.iter().collect();
    if pass % 2 == 1 {
        order.reverse();
    }
    order
}

fn answered(line: &mut Value, answer: &tinystore::Result<usize>) {
    match answer {
        Ok(hits) => {
            line["ok"] = json!(true);
            line["hits"] = json!(hits);
        }
        Err(error) => {
            line["ok"] = json!(false);
            line["error_kind"] = json!(format!("{:?}", error.kind()));
            line["error"] = json!(error.to_string());
        }
    }
}

fn measure(dir: &Path, memory: Option<u64>, heap_passes: usize, time_passes: usize) {
    let fixture: Value = serde_json::from_str(&fs::read_to_string(dir.join("fixture.json")).unwrap()).unwrap();
    let label = fixture["index"].as_str().unwrap().to_owned();
    let words = read_words(dir);
    let (arms, spreads) = arms(&words);
    let options = Options { background: false, memory, ..Options::default() };
    let store = Store::open(dir.join("store"), options).unwrap();
    let index = store.index::<String>(INDEX).open().unwrap();
    let mut out = std::io::stdout().lock();
    let described: Vec<Value> = arms
        .iter()
        .map(|arm| {
            let prefixes: Vec<Value> = arm
                .prefixes
                .iter()
                .map(|chosen| {
                    json!({"prefix": chosen.prefix, "begins": chosen.begins, "words": chosen.words.len(),
                        "postings": chosen.postings})
                })
                .collect();
            json!({"arm": arm.name, "text": arm.text, "mode": format!("{:?}", arm.mode), "prefixes": prefixes,
                "postings": arm.prefixes.iter().map(|chosen| chosen.postings).sum::<u64>()})
        })
        .collect();
    let realloc = if IN_PLACE.load(Ordering::SeqCst) { "in-place" } else { "copy" };
    let head = json!({"step": "arms", "index": label, "memory": memory, "arms": described, "spreads": spreads,
        "sqlite": compiled(), "realloc": realloc});
    writeln!(out, "{head}").unwrap();
    let line = |kind: &str, pass: usize, arm: &Arm| {
        json!({"step": "search", "index": label, "memory": memory, "kind": kind, "pass": pass, "arm": arm.name})
    };
    // each search once, so that what searches keep between them is kept
    for arm in &arms {
        let (answer, held) = measured(&store, || search(&index, arm));
        let mut first = line("first", 0, arm);
        answered(&mut first, &answer);
        heap(&mut first, &held);
        writeln!(out, "{first}").unwrap();
    }
    let rest = json!({"step": "rest", "index": label, "memory": memory, "heap": HELD.load(Ordering::SeqCst),
        "reserved": store.memory().used()});
    writeln!(out, "{rest}").unwrap();
    for pass in 0..heap_passes {
        for arm in ordered(&arms, pass) {
            let (answer, held) = measured(&store, || search(&index, arm));
            let mut heaped = line("heap", pass, arm);
            answered(&mut heaped, &answer);
            heap(&mut heaped, &held);
            writeln!(out, "{heaped}").unwrap();
        }
    }
    for pass in 0..time_passes {
        for arm in ordered(&arms, pass) {
            let started = Instant::now();
            let answer = search(&index, arm);
            let nanos = started.elapsed().as_nanos() as u64;
            let mut timed = line("time", pass, arm);
            answered(&mut timed, &answer);
            timed["nanos"] = json!(nanos);
            writeln!(out, "{timed}").unwrap();
        }
    }
    drop(index);
    store.close().unwrap();
}

/// The linked SQLite's version and whether TinyStore's flags reached it.
fn compiled() -> Value {
    let used = |option: &std::ffi::CStr| {
        // SAFETY: sqlite3_compileoption_used reads a table compiled into
        // SQLite, from any thread, and its argument lives for the call.
        unsafe { rusqlite::ffi::sqlite3_compileoption_used(option.as_ptr()) != 0 }
    };
    json!({"version": rusqlite::version(), "DQS=0": used(c"DQS=0"),
        "ENABLE_MEMORY_MANAGEMENT": used(c"ENABLE_MEMORY_MANAGEMENT"), "ENABLE_DBPAGE_VTAB": used(c"ENABLE_DBPAGE_VTAB")})
}

fn heap(line: &mut Value, held: &Held) {
    line["heap_peak"] = json!(held.peak);
    line["heap_retained"] = json!(held.retained);
    line["reserved"] = json!(held.reserved);
    line["sqlite_peak"] = json!(held.sqlite);
}

//! A directory scan as TinyStore's book of external directories plans it,
//! measured: the walk, the scan-local table SQLite sorts each folder's names
//! in, and the merge in the byte order of whole paths against a list of a row
//! a file, its differences written a thousand rows a commit.
//!
//! ```text
//! dir-scan-bench gen   <tree> lib:<files>|flat:<files> <seed>
//! dir-scan-bench walk  <tree> <label>
//! dir-scan-bench scan  <tree> <list.db> <label> [table|memory]
//! dir-scan-bench touch <tree> <per-mille> <seed>
//! dir-scan-bench size  <list.db> <label>
//! dir-scan-bench info
//! dir-scan-bench remove <tree>
//! ```
//!
//! Each command prints one JSON line to stdout.

#![allow(unsafe_code, reason = "the process's peak memory and CPU time on Windows")]

use rusqlite::{Connection, params};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const COMMIT_ROWS: usize = 1000;
const LIST_PAGE: usize = 1000;
const NAMES_A_TRANSACTION: usize = 4096;
const NAMES_PAGE: usize = 1000;
const DIRECTORY: i64 = 1;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or_else(|| usage());
    match arg(1) {
        "gen" => generate(Path::new(arg(2)), arg(3), arg(4).parse().unwrap()),
        "walk" => walk_command(Path::new(arg(2)), arg(3)),
        "scan" => scan_command(Path::new(arg(2)), Path::new(arg(3)), arg(4), args.get(5).map_or("table", |s| s)),
        "touch" => touch(Path::new(arg(2)), arg(3).parse().unwrap(), arg(4).parse().unwrap()),
        "size" => size(Path::new(arg(2)), arg(3)),
        "info" => info(),
        "remove" => remove(Path::new(arg(2))),
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("dir-scan-bench gen|walk|scan|touch|size|info …");
    std::process::exit(2)
}

// ---------------------------------------------------------------- fixtures

/// splitmix64: the fixture is the seed's alone.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn range(&mut self, low: u64, high: u64) -> u64 {
        low + self.next() % (high - low + 1)
    }
}

const WORDS: [&str; 12] = [
    "Blue", "Midnight Train", "Echoes", "the Long Road Home", "Static", "Paper Moon",
    "Northern Lights and Southern Skies", "Glass", "Ember", "A Quiet Place to Stand", "Tide", "Overture",
];
const EXTENSIONS: [&str; 6] = ["flac", "mp3", "mkv", "jpg", "nfo", "srt"];

struct Made {
    files: u64,
    folders: u64,
    bytes: u64,
}

/// A media library: leaf folders of 5 to 50 files at depth 2 to 4, each
/// folder above holding 3 to 20 folders; or one flat folder.
fn generate(tree: &Path, shape: &str, seed: u64) {
    let started = Instant::now();
    let (kind, files) = shape.split_once(':').unwrap_or_else(|| usage());
    let files: u64 = files.parse().unwrap();
    fs::create_dir_all(tree).unwrap();
    let mut rng = Rng(seed);
    let mut made = Made { files: 0, folders: 0, bytes: 0 };
    match kind {
        "lib" => generate_library(tree, files, &mut rng, &mut made),
        "flat" => {
            let folder = tree.join("flat");
            fs::create_dir(&folder).unwrap();
            made.folders += 1;
            for _ in 0..files {
                make_file(&folder, &mut rng, &mut made);
            }
        }
        _ => usage(),
    }
    println!(
        r#"{{"op":"gen","shape":"{shape}","seed":{seed},"files":{},"folders":{},"bytes":{},"seconds":{:.3}}}"#,
        made.files,
        made.folders,
        made.bytes,
        started.elapsed().as_secs_f64()
    );
}

fn generate_library(tree: &Path, files: u64, rng: &mut Rng, made: &mut Made) {
    // open[i] is the folder at level i + 1 still taking children, and how many
    let mut open: Vec<(PathBuf, u64)> = Vec::new();
    while made.files < files {
        if let Some(full) = open.iter().position(|(_, left)| *left == 0) {
            open.truncate(full);
        }
        let leaf_level = rng.range(2, 4) as usize;
        open.truncate(leaf_level - 1);
        while open.len() < leaf_level - 1 {
            let parent = match open.last_mut() {
                Some((path, left)) => {
                    *left -= 1;
                    path.clone()
                }
                None => tree.to_path_buf(),
            };
            let folder = parent.join(folder_name(open.len() + 1, rng, made));
            fs::create_dir(&folder).unwrap();
            made.folders += 1;
            let children = rng.range(3, 20);
            open.push((folder, children));
        }
        let parent = match open.last_mut() {
            Some((path, left)) => {
                *left -= 1;
                path.clone()
            }
            None => tree.to_path_buf(),
        };
        let leaf = parent.join(folder_name(leaf_level, rng, made));
        fs::create_dir(&leaf).unwrap();
        made.folders += 1;
        let count = rng.range(5, 50).min(files - made.files);
        for _ in 0..count {
            make_file(&leaf, rng, made);
        }
    }
}

fn folder_name(level: usize, rng: &mut Rng, made: &Made) -> String {
    let word = WORDS[rng.next() as usize % WORDS.len()];
    match level {
        1 => format!("Artist {:06} {word}", made.folders),
        2 => format!("{} - {word} ({:06})", 1960 + rng.next() % 66, made.folders),
        3 => format!("Disc {} {:06}", 1 + rng.next() % 4, made.folders),
        _ => format!("Extras {:06} {word}", made.folders),
    }
}

fn make_file(folder: &Path, rng: &mut Rng, made: &mut Made) {
    let word = WORDS[rng.next() as usize % WORDS.len()];
    let extension = EXTENSIONS[rng.next() as usize % EXTENSIONS.len()];
    let name = format!("{:02} - {word} {:07}.{extension}", 1 + rng.next() % 30, made.files);
    let size = rng.range(0, 4096);
    // 2020-01-01 plus up to five years, to the second
    let time = UNIX_EPOCH + Duration::from_secs(1_577_836_800 + rng.next() % (5 * 365 * 86_400));
    let file = fs::File::create(folder.join(name)).unwrap();
    file.set_len(size).unwrap();
    file.set_modified(time).unwrap();
    made.files += 1;
    made.bytes += size;
}

/// Sets the time of about `per_mille` thousandths of the files, chosen by a
/// hash of the path and the seed, to a time no file had.
fn touch(tree: &Path, per_mille: u64, seed: u64) {
    let started = Instant::now();
    let stamp = SystemTime::now();
    let mut touched = 0u64;
    let mut stack = vec![tree.to_path_buf()];
    while let Some(folder) = stack.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            let path = entry.path();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && fnv(path.as_os_str().as_encoded_bytes(), seed) % 1000 < per_mille {
                let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
                file.set_modified(stamp).unwrap();
                touched += 1;
            }
        }
    }
    println!(
        r#"{{"op":"touch","per_mille":{per_mille},"seed":{seed},"touched":{touched},"seconds":{:.3}}}"#,
        started.elapsed().as_secs_f64()
    );
}

fn fnv(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ seed.wrapping_mul(0x100_0000_01b3);
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
    }
    let mut mix = Rng(hash);
    mix.next()
}

// ---------------------------------------------------------------- the walk

/// What a folder's entry is to a scan.
enum Kind {
    File { size: i64, mtime: i64 },
    Folder,
    /// A link, a folder on another file system or one std could not stat.
    Skipped,
}

/// The device a scan stays on: `st_dev` on Unix. On Windows a volume mounted
/// in a folder is a reparse point, which `kind` refuses from the listing's
/// attributes, so there is nothing to compare.
#[cfg(unix)]
type Device = u64;
#[cfg(windows)]
type Device = ();

#[cfg(unix)]
fn device_of(path: &Path) -> Device {
    use std::os::unix::fs::MetadataExt;
    fs::symlink_metadata(path).unwrap().dev()
}
#[cfg(windows)]
fn device_of(_path: &Path) -> Device {}

/// What std gives for one entry: on Windows the listing's own size, time and
/// attributes; on Linux a stat of the entry, which `file_type` alone spares.
fn kind(entry: &fs::DirEntry, device: &Device) -> Kind {
    let Ok(file_type) = entry.file_type() else { return Kind::Skipped };
    if file_type.is_symlink() {
        return Kind::Skipped;
    }
    let Ok(metadata) = entry.metadata() else { return Kind::Skipped };
    if crosses(&metadata, device) {
        return Kind::Skipped;
    }
    if file_type.is_dir() {
        Kind::Folder
    } else if file_type.is_file() {
        Kind::File { size: metadata.len() as i64, mtime: nanos(metadata.modified().ok()) }
    } else {
        Kind::Skipped
    }
}

#[cfg(unix)]
fn crosses(metadata: &fs::Metadata, device: &Device) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.is_dir() && metadata.dev() != *device
}
#[cfg(windows)]
fn crosses(metadata: &fs::Metadata, _device: &Device) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn nanos(time: Option<SystemTime>) -> i64 {
    match time.map(|t| t.duration_since(UNIX_EPOCH)) {
        Some(Ok(since)) => since.as_nanos() as i64,
        _ => 0,
    }
}

/// A name as the list keeps it, UTF-8 bytes, or `None` for one that is not.
fn name_bytes(entry: &fs::DirEntry) -> Option<Vec<u8>> {
    entry.file_name().to_str().map(|name| name.as_bytes().to_vec())
}

#[derive(Default)]
struct Counts {
    files: u64,
    bytes: u64,
    folders: u64,
    skipped: u64,
    added: u64,
    changed: u64,
    removed: u64,
    commits: u64,
}

fn walk_command(tree: &Path, label: &str) {
    let started = Instant::now();
    let device = device_of(tree);
    let mut counts = Counts::default();
    walk(tree, &device, &mut counts);
    let seconds = started.elapsed().as_secs_f64();
    let usage = Usage::now();
    println!(
        r#"{{"op":"walk","label":"{label}","platform":"{}","seconds":{seconds:.4},"cpu_seconds":{:.4},"peak_rss":{},"files":{},"folders":{},"skipped":{},"bytes":{}}}"#,
        std::env::consts::OS,
        usage.cpu_seconds,
        usage.peak_rss,
        counts.files,
        counts.folders,
        counts.skipped,
        counts.bytes
    );
}

fn walk(folder: &Path, device: &Device, counts: &mut Counts) {
    counts.folders += 1;
    let Ok(entries) = fs::read_dir(folder) else {
        counts.skipped += 1;
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            counts.skipped += 1;
            continue;
        };
        if name_bytes(&entry).is_none() {
            counts.skipped += 1;
            continue;
        }
        match kind(&entry, device) {
            Kind::File { size, .. } => {
                counts.files += 1;
                counts.bytes += size as u64;
            }
            Kind::Folder => walk(&entry.path(), device, counts),
            Kind::Skipped => counts.skipped += 1,
        }
    }
}

// ---------------------------------------------------------------- the list

const LIST_SCHEMA: &str = "
create table if not exists files (
  directory integer not null,
  path blob not null,
  size integer not null,
  mtime integer not null,
  change integer not null,
  removed integer,
  primary key (directory, path)
) strict, without rowid;
create index if not exists files_by_change on files (directory, change);
create table if not exists changes (directory integer primary key, last integer not null) strict;
";
const LIST_PAGE_SQL: &str = "select path, size, mtime, removed is not null from files
  where directory = ?1 and path > ?2 order by path limit cast(?3 as integer)";
const UPSERT: &str = "insert into files (directory, path, size, mtime, change, removed)
  values (?1, ?2, ?3, ?4, ?5, null)
  on conflict (directory, path) do update set
    size = excluded.size, mtime = excluded.mtime, change = excluded.change, removed = null";
const MARK_REMOVED: &str = "update files set removed = ?3, change = ?4 where directory = ?1 and path = ?2";
const LAST_CHANGE: &str = "select last from changes where directory = ?1";
const SET_LAST_CHANGE: &str = "insert into changes (directory, last) values (?1, ?2)
  on conflict (directory) do update set last = excluded.last";

/// The list's connection as TinyStore opens a writer by default: 4 KiB pages,
/// WAL, synchronous FULL, a cache of 1 MiB. SQLite's own checkpoint at 1000
/// frames runs inside the commit here, where TinyStore runs it beside the
/// writer: its cost is in this scan's time rather than another caller's.
fn open_list(path: &Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection.pragma_update(None, "page_size", 4096).unwrap();
    let mode: String = connection.pragma_update_and_check(None, "journal_mode", "wal", |row| row.get(0)).unwrap();
    assert_eq!(mode, "wal");
    connection.pragma_update(None, "synchronous", "full").unwrap();
    connection.pragma_update(None, "foreign_keys", true).unwrap();
    connection.pragma_update(None, "cache_size", -1024).unwrap();
    connection.execute_batch(LIST_SCHEMA).unwrap();
    connection
}

struct Stored {
    path: Vec<u8>,
    size: i64,
    mtime: i64,
    removed: bool,
}

/// The stored list read in path order a page at a time, so that what it
/// holds in memory is one page, whatever the list's length.
struct ListCursor {
    page: VecDeque<Stored>,
    after: Vec<u8>,
    ended: bool,
}

impl ListCursor {
    fn new() -> Self {
        Self { page: VecDeque::new(), after: Vec::new(), ended: false }
    }

    fn peek(&mut self, list: &Connection) -> Option<&Stored> {
        if self.page.is_empty() && !self.ended {
            let mut statement = list.prepare_cached(LIST_PAGE_SQL).unwrap();
            let rows = statement
                .query_map(params![DIRECTORY, self.after, LIST_PAGE as i64], |row| {
                    Ok(Stored { path: row.get(0)?, size: row.get(1)?, mtime: row.get(2)?, removed: row.get(3)? })
                })
                .unwrap();
            for row in rows {
                self.page.push_back(row.unwrap());
            }
            if self.page.len() < LIST_PAGE {
                self.ended = true;
            }
            if let Some(last) = self.page.back() {
                self.after = last.path.clone();
            }
        }
        self.page.front()
    }

    fn pop(&mut self) -> Stored {
        self.page.pop_front().unwrap()
    }
}

enum Write {
    Put { path: Vec<u8>, size: i64, mtime: i64 },
    Remove { path: Vec<u8> },
}

/// The scan's differences, written a thousand rows a commit.
struct Writer {
    pending: Vec<Write>,
    last_change: i64,
    now: i64,
}

impl Writer {
    fn push(&mut self, list: &mut Connection, write: Write, counts: &mut Counts) {
        self.pending.push(write);
        if self.pending.len() == COMMIT_ROWS {
            self.flush(list, counts);
        }
    }

    fn flush(&mut self, list: &mut Connection, counts: &mut Counts) {
        if self.pending.is_empty() {
            return;
        }
        let tx = list.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).unwrap();
        {
            let mut put = tx.prepare_cached(UPSERT).unwrap();
            let mut remove = tx.prepare_cached(MARK_REMOVED).unwrap();
            for write in self.pending.drain(..) {
                self.last_change += 1;
                match write {
                    Write::Put { path, size, mtime } => {
                        put.execute(params![DIRECTORY, path, size, mtime, self.last_change]).unwrap();
                    }
                    Write::Remove { path } => {
                        remove.execute(params![DIRECTORY, path, self.now, self.last_change]).unwrap();
                    }
                }
            }
            tx.execute(SET_LAST_CHANGE, params![DIRECTORY, self.last_change]).unwrap();
        }
        tx.commit().unwrap();
        counts.commits += 1;
    }
}

// ---------------------------------------------------------------- the scan

/// A folder's entries, sorted in the byte order of whole paths: a folder's
/// key is its name and a `/`, so that what is under it sorts where its
/// paths do (`a-c` < `a/b` < `a0`).
struct Name {
    key: Vec<u8>,
    kind: Kind,
}

/// Where a folder's names are sorted: SQLite's table, or a Vec in memory,
/// the candidate that holds a whole folder at once.
enum Sorter {
    Table(Connection),
    Memory,
}

const NAMES_SCHEMA: &str = "
create table names (
  depth integer not null,
  key blob not null,
  kind integer not null,
  size integer not null,
  mtime integer not null,
  primary key (depth, key)
) strict, without rowid;
";
const NAMES_INSERT: &str = "insert into names values (?1, ?2, ?3, ?4, ?5)";
const NAMES_PAGE_SQL: &str = "select key, kind, size, mtime from names
  where depth = ?1 and key > ?2 order by key limit cast(?3 as integer)";
const NAMES_CLEAR: &str = "delete from names where depth = ?1";

/// A private temporary database: SQLite keeps its pages in a cache of
/// 2 MiB and spills the rest to a file of its own it deletes at close.
fn open_names() -> Connection {
    let names = Connection::open("").unwrap();
    names.pragma_update(None, "journal_mode", "off").unwrap();
    names.pragma_update(None, "synchronous", "off").unwrap();
    names.pragma_update(None, "cache_size", -2048).unwrap();
    names.pragma_update(None, "temp_store", "file").unwrap();
    names.execute_batch(NAMES_SCHEMA).unwrap();
    names
}

struct Scan<'a> {
    list: &'a mut Connection,
    sorter: Sorter,
    cursor: ListCursor,
    writer: Writer,
    counts: Counts,
    device: Device,
}

fn scan_command(tree: &Path, list_path: &Path, label: &str, sort: &str) {
    let started = Instant::now();
    let mut list = open_list(list_path);
    let last_change = list.query_row(LAST_CHANGE, [DIRECTORY], |row| row.get(0)).unwrap_or(0);
    let sorter = match sort {
        "table" => Sorter::Table(open_names()),
        "memory" => Sorter::Memory,
        _ => usage(),
    };
    let now = nanos(Some(SystemTime::now()));
    let mut scan = Scan {
        list: &mut list,
        sorter,
        cursor: ListCursor::new(),
        writer: Writer { pending: Vec::new(), last_change, now },
        counts: Counts::default(),
        device: device_of(tree),
    };
    scan.folder(tree, &[], 0);
    scan.remove_rest(None);
    let Scan { list, writer, mut counts, .. } = scan;
    let mut writer = writer;
    writer.flush(list, &mut counts);
    let seconds = started.elapsed().as_secs_f64();
    let usage = Usage::now();
    println!(
        r#"{{"op":"scan","label":"{label}","sort":"{sort}","platform":"{}","seconds":{seconds:.4},"cpu_seconds":{:.4},"peak_rss":{},"files":{},"folders":{},"skipped":{},"added":{},"changed":{},"removed":{},"commits":{}}}"#,
        std::env::consts::OS,
        usage.cpu_seconds,
        usage.peak_rss,
        counts.files,
        counts.folders,
        counts.skipped,
        counts.added,
        counts.changed,
        counts.removed,
        counts.commits
    );
}

impl Scan<'_> {
    /// Scans one folder whose path in the list is `prefix` (empty, or ending
    /// in `/`), then the folders in it at their places in its order.
    fn folder(&mut self, path: &Path, prefix: &[u8], depth: i64) {
        self.counts.folders += 1;
        let Ok(entries) = fs::read_dir(path) else {
            assert!(depth > 0, "the directory's own folder cannot be read");
            self.counts.skipped += 1;
            self.keep_under(prefix);
            return;
        };
        match &self.sorter {
            Sorter::Table(_) => self.folder_by_table(entries, path, prefix, depth),
            Sorter::Memory => self.folder_in_memory(entries, path, prefix, depth),
        }
    }

    fn folder_by_table(&mut self, entries: fs::ReadDir, path: &Path, prefix: &[u8], depth: i64) {
        self.read_names(entries, depth);
        let mut after: Vec<u8> = Vec::new();
        loop {
            let page = self.names_page(depth, &after);
            let Some(last) = page.last() else { break };
            after = last.key.clone();
            let full = page.len() == NAMES_PAGE;
            for name in page {
                self.visit(name, path, prefix, depth);
            }
            if !full {
                break;
            }
        }
        let Sorter::Table(names) = &self.sorter else { unreachable!() };
        names.prepare_cached(NAMES_CLEAR).unwrap().execute([depth]).unwrap();
    }

    fn folder_in_memory(&mut self, entries: fs::ReadDir, path: &Path, prefix: &[u8], depth: i64) {
        let mut names = Vec::new();
        for entry in entries {
            if let Some(name) = self.name(entry) {
                names.push(name);
            }
        }
        names.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        for name in names {
            self.visit(name, path, prefix, depth);
        }
    }

    /// Reads a folder's names into the table a few thousand a transaction.
    fn read_names(&mut self, entries: fs::ReadDir, depth: i64) {
        let mut batch: Vec<Name> = Vec::with_capacity(NAMES_A_TRANSACTION);
        for entry in entries {
            if let Some(name) = self.name(entry) {
                batch.push(name);
            }
            if batch.len() == NAMES_A_TRANSACTION {
                self.insert_names(&mut batch, depth);
            }
        }
        self.insert_names(&mut batch, depth);
    }

    fn insert_names(&mut self, batch: &mut Vec<Name>, depth: i64) {
        let Sorter::Table(names) = &mut self.sorter else { unreachable!() };
        let tx = names.transaction().unwrap();
        {
            let mut insert = tx.prepare_cached(NAMES_INSERT).unwrap();
            for name in batch.drain(..) {
                let (kind, size, mtime) = match name.kind {
                    Kind::File { size, mtime } => (0, size, mtime),
                    Kind::Folder => (1, 0, 0),
                    Kind::Skipped => (2, 0, 0),
                };
                insert.execute(params![depth, name.key, kind, size, mtime]).unwrap();
            }
        }
        tx.commit().unwrap();
    }

    fn names_page(&mut self, depth: i64, after: &[u8]) -> Vec<Name> {
        let Sorter::Table(names) = &self.sorter else { unreachable!() };
        let mut statement = names.prepare_cached(NAMES_PAGE_SQL).unwrap();
        let rows = statement
            .query_map(params![depth, after, NAMES_PAGE as i64], |row| {
                let kind = match row.get::<_, i64>(1)? {
                    0 => Kind::File { size: row.get(2)?, mtime: row.get(3)? },
                    1 => Kind::Folder,
                    _ => Kind::Skipped,
                };
                Ok(Name { key: row.get(0)?, kind })
            })
            .unwrap();
        rows.map(Result::unwrap).collect()
    }

    /// A listed entry as a name to sort, or `None` for one that is not text.
    /// A folder skipped stays among the names, so that the list's files under
    /// it are kept at its place rather than told removed.
    fn name(&mut self, entry: std::io::Result<fs::DirEntry>) -> Option<Name> {
        let Ok(entry) = entry else {
            self.counts.skipped += 1;
            return None;
        };
        let Some(mut key) = name_bytes(&entry) else {
            self.counts.skipped += 1;
            return None;
        };
        let kind = kind(&entry, &self.device);
        if matches!(kind, Kind::Folder | Kind::Skipped) {
            key.push(b'/');
        }
        Some(Name { key, kind })
    }

    fn visit(&mut self, name: Name, path: &Path, prefix: &[u8], depth: i64) {
        let mut full = Vec::with_capacity(prefix.len() + name.key.len());
        full.extend_from_slice(prefix);
        full.extend_from_slice(&name.key);
        match name.kind {
            Kind::File { size, mtime } => self.file(full, size, mtime),
            Kind::Folder => {
                let child = path.join(std::str::from_utf8(&name.key[..name.key.len() - 1]).unwrap());
                self.folder(&child, &full, depth + 1);
            }
            Kind::Skipped => {
                self.counts.skipped += 1;
                self.keep_under(&full);
            }
        }
    }

    /// Merges one file of the walk against the list.
    fn file(&mut self, path: Vec<u8>, size: i64, mtime: i64) {
        self.counts.files += 1;
        self.counts.bytes += size as u64;
        self.remove_rest(Some(&path));
        let same = match self.cursor.peek(self.list) {
            Some(stored) if stored.path == path => {
                let stored = self.cursor.pop();
                if stored.removed {
                    Some(false)
                } else {
                    Some(stored.size == size && stored.mtime == mtime)
                }
            }
            _ => None,
        };
        match same {
            Some(true) => {}
            Some(false) => {
                self.counts.changed += 1;
                self.writer.push(self.list, Write::Put { path, size, mtime }, &mut self.counts);
            }
            None => {
                self.counts.added += 1;
                self.writer.push(self.list, Write::Put { path, size, mtime }, &mut self.counts);
            }
        }
    }

    /// Tells removed every listed file before `until`, or all that are left.
    fn remove_rest(&mut self, until: Option<&[u8]>) {
        loop {
            let Some(stored) = self.cursor.peek(self.list) else { return };
            if until.is_some_and(|until| stored.path.as_slice() >= until) {
                return;
            }
            let stored = self.cursor.pop();
            if !stored.removed {
                self.counts.removed += 1;
                self.writer.push(self.list, Write::Remove { path: stored.path }, &mut self.counts);
            }
        }
    }

    /// Passes over the listed files under a folder the scan did not read.
    fn keep_under(&mut self, prefix: &[u8]) {
        self.remove_rest(Some(prefix));
        while self.cursor.peek(self.list).is_some_and(|stored| stored.path.starts_with(prefix)) {
            self.cursor.pop();
        }
    }
}

// ---------------------------------------------------------------- storage

/// The list's file divided by b-tree, after a checkpoint that empties the WAL.
fn size(list_path: &Path, label: &str) {
    let list = Connection::open(list_path).unwrap();
    list.query_row("pragma wal_checkpoint(truncate)", [], |_| Ok(())).unwrap();
    let one = |sql: &str| list.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap();
    let page_size = one("pragma page_size");
    let pages = one("pragma page_count");
    let free = one("pragma freelist_count");
    let rows = one("select count(*) from files");
    let payload = one("select sum(length(path)) + 0 from files");
    let mut statement = list
        .prepare(
            "select name, count(*), sum(ncell), sum(payload), sum(unused), sum(pgsize)
             from dbstat group by name order by name",
        )
        .unwrap();
    let objects: Vec<String> = statement
        .query_map([], |row| {
            Ok(format!(
                r#"{{"name":"{}","pages":{},"cells":{},"payload":{},"unused":{},"bytes":{}}}"#,
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let file_bytes = fs::metadata(list_path).unwrap().len();
    println!(
        r#"{{"op":"size","label":"{label}","rows":{rows},"page_size":{page_size},"pages":{pages},"free_pages":{free},"file_bytes":{file_bytes},"path_bytes":{payload},"objects":[{}]}}"#,
        objects.join(",")
    );
}

/// Removes a fixture, faster on Windows than a shell's `rm -rf`.
fn remove(tree: &Path) {
    let started = Instant::now();
    if tree.exists() {
        fs::remove_dir_all(tree).unwrap();
    }
    println!(r#"{{"op":"remove","seconds":{:.3}}}"#, started.elapsed().as_secs_f64());
}

fn info() {
    println!(
        r#"{{"op":"info","platform":"{}","arch":"{}","sqlite":"{}","rusqlite":"0.40.2"}}"#,
        std::env::consts::OS,
        std::env::consts::ARCH,
        rusqlite::version()
    );
}

// ---------------------------------------------------------------- the process

struct Usage {
    cpu_seconds: f64,
    peak_rss: u64,
}

#[cfg(target_os = "linux")]
impl Usage {
    /// VmHWM from /proc/self/status, user and system time from
    /// /proc/self/stat in ticks of 1/100 s, the kernel's USER_HZ.
    fn now() -> Self {
        let status = fs::read_to_string("/proc/self/status").unwrap();
        let peak_kib: u64 = status
            .lines()
            .find_map(|line| line.strip_prefix("VmHWM:"))
            .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap_or(0);
        let stat = fs::read_to_string("/proc/self/stat").unwrap();
        let after_name = &stat[stat.rfind(')').unwrap() + 2..];
        let fields: Vec<&str> = after_name.split(' ').collect();
        // utime and stime are the stat's 14th and 15th fields, 12th and 13th after the name
        let ticks: u64 = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
        Self { cpu_seconds: ticks as f64 / 100.0, peak_rss: peak_kib * 1024 }
    }
}

#[cfg(windows)]
mod win {
    #[repr(C)]
    #[derive(Default)]
    pub struct MemoryCounters {
        pub cb: u32,
        pub page_fault_count: u32,
        pub peak_working_set_size: usize,
        pub working_set_size: usize,
        pub quota_peak_paged_pool_usage: usize,
        pub quota_paged_pool_usage: usize,
        pub quota_peak_non_paged_pool_usage: usize,
        pub quota_non_paged_pool_usage: usize,
        pub pagefile_usage: usize,
        pub peak_pagefile_usage: usize,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct FileTime {
        pub low: u32,
        pub high: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetCurrentProcess() -> isize;
        pub fn K32GetProcessMemoryInfo(process: isize, counters: *mut MemoryCounters, cb: u32) -> i32;
        pub fn GetProcessTimes(
            process: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }
}

#[cfg(windows)]
impl Usage {
    /// The peak working set and the kernel and user time, in 100 ns units.
    fn now() -> Self {
        let mut counters = win::MemoryCounters {
            cb: size_of::<win::MemoryCounters>() as u32,
            ..Default::default()
        };
        let (mut creation, mut exit, mut kernel, mut user) = Default::default();
        // SAFETY: the pseudo handle needs no closing; each pointer is to a
        // local of the layout the call writes, `cb` its size.
        unsafe {
            let process = win::GetCurrentProcess();
            assert!(win::K32GetProcessMemoryInfo(process, &mut counters, counters.cb) != 0);
            assert!(win::GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) != 0);
        }
        let hundreds = |t: &win::FileTime| (u64::from(t.high) << 32) | u64::from(t.low);
        let _ = (&creation, &exit);
        Self {
            cpu_seconds: (hundreds(&kernel) + hundreds(&user)) as f64 / 1e7,
            peak_rss: counters.peak_working_set_size as u64,
        }
    }
}

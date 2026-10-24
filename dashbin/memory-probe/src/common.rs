// what every probe reports: resident memory from /proc, split into heap and mapped files
#![allow(dead_code)]

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub fn allocator() -> &'static str {
    if cfg!(feature = "mimalloc") {
        "mimalloc"
    } else if cfg!(feature = "jemalloc") {
        "jemalloc"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else {
        "glibc"
    }
}

pub fn rss() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let pick = |key: &str| {
        status
            .lines()
            .find(|l| l.starts_with(key))
            .map(|l| l[key.len()..].trim().trim_end_matches(" kB").trim().parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0)
    };
    format!(
        "VmRSS {:.1} MiB  RssAnon {:.1}  RssFile {:.1}  VmHWM {:.1}  threads {}",
        pick("VmRSS:") / 1024.0,
        pick("RssAnon:") / 1024.0,
        pick("RssFile:") / 1024.0,
        pick("VmHWM:") / 1024.0,
        pick("Threads:")
    )
}

#[cfg(all(target_env = "gnu", not(any(feature = "mimalloc", feature = "jemalloc"))))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

// asks the allocator to hand its free pages back now, by whatever call it has
pub fn trim() -> &'static str {
    #[cfg(feature = "mimalloc")]
    {
        unsafe { libmimalloc_sys::mi_collect(true) };
        return "mi_collect(true)";
    }
    #[cfg(feature = "jemalloc")]
    {
        unsafe {
            tikv_jemalloc_sys::mallctl(c"arena.4096.purge".as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0)
        };
        return "arena.all.purge";
    }
    #[cfg(all(target_env = "gnu", not(any(feature = "mimalloc", feature = "jemalloc"))))]
    {
        unsafe { malloc_trim(0) };
        return "malloc_trim(0)";
    }
    #[allow(unreachable_code)]
    "none"
}

pub fn rss_after_trim() -> String {
    let how = trim();
    format!("{}  [{how}]", rss())
}

pub fn checkpoint(what: &str) {
    println!("{what:<44} {}", rss());
}

// the same work a third-party plugin would do: parse a JSON answer of 10k objects into two columns
pub const PLUGIN: &str = r#"
function query(n) {
  const time = new Float64Array(n), value = new Float64Array(n);
  const t0 = Date.now();
  for (let i = 0; i < n; i++) { time[i] = t0 - (n - i) * 15000; value[i] = Math.sin(i / 10) * 100 + 200; }
  const body = JSON.stringify(Array.from({ length: n }, (_, i) => ({ created: time[i] / 1000, amount: value[i] })));
  const data = JSON.parse(body);
  let sum = 0;
  for (const r of data) sum += r.amount;
  return sum;
}
"#;

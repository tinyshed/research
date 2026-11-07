//! The fast path: what `pairs_named` and `graph_features` compute, without a
//! string allocated for every pair or a column's rivals walked for every
//! target. Checked against them, pair by pair, by `reader selfcheck`.

use crate::*;
use std::collections::HashMap;

const NAMES: [&str; 23] = [
    "stem=target", "stem=target singular", "stem in target", "target in stem", "named as target's key",
    "shared tokens", "ends in id", "self", "target columns", "v2 letters", "v2 edit", "v2 prefix", "v2 base",
    "v2 best of the column", "v2 margin", "v2 close rivals", "v2 named as the key", "v2 same type", "v2 named by",
    "v2 id convention", "v2 bare", "v3 own key", "v3 pseudo target",
];

fn constant(k: usize) -> u16 {
    static TABLE: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| NAMES.iter().map(|n| index_of(n)).collect())[k]
}

/// keys.py's `grams` as codes: three code points of 21 bits each, so the
/// codes sort as the strings do.
fn code_grams(s: &str) -> Vec<u64> {
    let chars: Vec<char> = std::iter::once('^').chain(s.chars()).chain(std::iter::once('$')).collect();
    let mut out: Vec<u64> =
        chars.windows(3).map(|w| ((w[0] as u64) << 42) | ((w[1] as u64) << 21) | w[2] as u64).collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn code_string(c: u64) -> String {
    [(c >> 42) & 0x1f_ffff, (c >> 21) & 0x1f_ffff, c & 0x1f_ffff]
        .iter()
        .map(|&x| char::from_u32(x as u32).unwrap_or('\u{fffd}'))
        .collect()
}

fn jaccard_codes(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let (mut i, mut j, mut both) = (0, 0, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                both += 1;
                i += 1;
                j += 1;
            }
        }
    }
    both as f64 / (a.len() + b.len() - both) as f64
}

fn similarity_in(a: &[char], b: &[char], prev: &mut Vec<usize>, cur: &mut Vec<usize>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    prev.clear();
    prev.extend(0..=b.len());
    for (i, &ca) in a.iter().enumerate() {
        cur.clear();
        cur.push(i + 1);
        for (j, &cb) in b.iter().enumerate() {
            let v = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(ca != cb));
            cur.push(v);
        }
        std::mem::swap(prev, cur);
    }
    1.0 - prev[b.len()] as f64 / a.len().max(b.len()) as f64
}

struct Fast<'a> {
    table: usize,
    kind: &'static str,
    name: String,
    singular: String,
    singular_chars: Vec<char>,
    singular3: String,
    grams: Vec<u64>,
    pk: String,
    pk_type: Option<&'a str>,
    key: &'a str,
    pseudo: bool,
    hashed: Vec<(u16, f64)>,
    last_tokens: Vec<String>,
    tokens: Vec<String>,
    columns: usize,
}

fn row_of(mut v: Vec<(u16, f64)>) -> Vec<(u16, f32)> {
    v.sort_by_key(|e| e.0); // stable: values of one index keep their order
    let mut out: Vec<(u16, f32)> = Vec::with_capacity(v.len());
    let mut i = 0;
    while i < v.len() {
        let mut sum = v[i].1;
        let mut j = i + 1;
        while j < v.len() && v[j].0 == v[i].0 {
            sum += v[j].1;
            j += 1;
        }
        out.push((v[i].0, sum as f32));
        i = j;
    }
    out
}

/// Every candidate pair of a schema, in widen.py's order, with its features.
pub fn pairs(s: &Schema) -> Vec<Pair> {
    let n_tables = s.tables.len();
    let mut targets: Vec<Fast> = Vec::new();
    for (ti, t) in s.tables.iter().enumerate() {
        let Some((key, pseudo)) = key_of(t) else { continue };
        let g = norm(&t.name);
        let sg = singular(&g);
        let gt = tokens(&t.name);
        let mut named = Named::default();
        for tok in &gt {
            named.set(format!("g:{tok}"), 1.0);
        }
        let mut grams = code_grams(&g);
        grams.extend(code_grams(&sg));
        grams.sort_unstable();
        grams.dedup();
        targets.push(Fast {
            table: ti,
            kind: kind(&key.family).unwrap(),
            grams,
            singular_chars: sg.chars().collect(),
            singular3: first_chars(&sg, 3),
            name: g,
            singular: sg,
            pk: norm(&key.name),
            pk_type: key.ty.as_deref(),
            key: &key.name,
            pseudo,
            hashed: hashed(&named),
            last_tokens: gt[gt.len().saturating_sub(2)..].to_vec(),
            tokens: gt,
            columns: t.columns.len(),
        });
    }
    let stems: Vec<String> = s.tables.iter().flat_map(|t| t.columns.iter().map(|c| stem_of(&c.name).0)).collect();
    let named_by: Vec<f64> = targets
        .iter()
        .map(|g| {
            stems.iter().filter(|st| !st.is_empty() && (**st == g.name || **st == g.singular)).count().min(10) as f64
                / 10.0
        })
        .collect();
    let mut n_columns = 0usize;
    let mut id_columns = 0usize;
    for t in &s.tables {
        for c in &t.columns {
            n_columns += 1;
            id_columns += usize::from(tokens(&c.name).last().is_some_and(|l| l == "id"));
        }
    }
    let id_convention = id_columns as f64 / n_columns.max(1) as f64;

    let mut out = Vec::new();
    let mut buf: Vec<u8> = Vec::with_capacity(96);
    let (mut prev, mut cur) = (Vec::new(), Vec::new());
    for (ti, table) in s.tables.iter().enumerate() {
        let table_tokens = tokens(&table.name);
        for (ci, column) in table.columns.iter().enumerate() {
            let Some(ck) = kind(&column.family) else { continue };
            let mine: Vec<usize> = (0..targets.len())
                .filter(|&k| targets[k].kind == ck && !(targets[k].table == ti && column.name == targets[k].key))
                .collect();
            if mine.is_empty() {
                continue;
            }
            let own = column.primary() && table.pk.len() == 1;
            let (stem, tk) = stem_of(&column.name);
            let stem_chars: Vec<char> = stem.chars().collect();
            let sg = if stem.is_empty() { Vec::new() } else { code_grams(&stem) };
            let cname = norm(&column.name);
            let base: Vec<f64> = mine
                .iter()
                .map(|&k| {
                    let g = &targets[k];
                    let exact = stem == g.name
                        || stem == g.singular
                        || (g.name.len() == stem.len() + 1 && g.name.starts_with(stem.as_str()) && g.name.ends_with('s'));
                    let p = prefix(&stem_chars, &g.singular_chars);
                    let pre = if p >= 3 {
                        0.8 * p as f64 / 1usize.max(stem_chars.len().min(g.singular_chars.len())) as f64
                    } else {
                        0.0
                    };
                    f64::from(u8::from(exact))
                        .max(jaccard_codes(&sg, &g.grams))
                        .max(f64::from(u8::from(cname == g.pk)))
                        .max(pre)
                })
                .collect();
            // The column's best, how many share it, the best of the rest and
            // how many reach a half: enough for every target's rivals
            let best = base.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let at_best = base.iter().filter(|&&x| x == best).count();
            let first_best = base.iter().position(|&x| x == best).unwrap();
            let second = base
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != first_best)
                .map(|(_, &x)| x)
                .fold(f64::NEG_INFINITY, f64::max);
            let halves = base.iter().filter(|&&x| x >= 0.5).count();

            let mut col = Named::default();
            for t in &tk {
                col.set(format!("c:{t}"), 1.0);
            }
            if let (Some(first), Some(last)) = (tk.first(), tk.last()) {
                col.set(format!("last:{last}"), 1.0);
                col.set(format!("first:{first}"), 1.0);
            }
            for t in &table_tokens {
                col.set(format!("t:{t}"), 1.0);
            }
            col.set(format!("family:{}", column.family), 1.0);
            col.set(format!("nullable:{}", py_bool(column.nullable)), 1.0);
            if column.unique() {
                col.set("unique".into(), 1.0);
            }
            if column.primary() {
                col.set(if table.pk.len() == 1 { "pk_single" } else { "pk_part" }.into(), 1.0);
            }
            if column.index.unwrap_or(false) {
                col.set("index".into(), 1.0);
            }
            col.set("tokens".into(), tk.len().min(6) as f64 / 6.0);
            col.set("position".into(), column.position.min(30) as f64 / 30.0);
            col.set("columns".into(), table.columns.len().min(60) as f64 / 60.0);
            col.set("tables".into(), n_tables.min(100) as f64 / 100.0);
            let col_hashed = hashed(&col);
            let key_word = index_of(&format!("v2 key word:{}", tk.last().map(String::as_str).unwrap_or("")));
            let letters: Vec<(u16, f64)> =
                sg.iter().take(12).map(|&c| (index_of(&format!("cg:{}", code_string(c))), 1.0)).collect();
            let stem_tokens: &[String] = if tk.last().is_some_and(|l| l == "id") { &tk[..tk.len() - 1] } else { &tk[..] };
            let stem2 = stem_tokens.concat();
            let mut stem_set: Vec<&str> = stem_tokens.iter().map(String::as_str).filter(|t| *t != "id").collect();
            stem_set.sort_unstable();
            stem_set.dedup();
            let last_stem = &stem_tokens[stem_tokens.len().saturating_sub(2)..];
            let ends_id = f64::from(u8::from(tk.last().is_some_and(|l| l == "id")));
            let stem3 = first_chars(&stem, 3);

            for (j, &k) in mine.iter().enumerate() {
                let g = &targets[k];
                let b = base[j];
                let max_others = if mine.len() == 1 {
                    0.0
                } else if b == best && at_best == 1 {
                    second
                } else {
                    best
                };
                let rivals = halves - usize::from(b >= 0.5);
                let mut v: Vec<(u16, f64)> = Vec::with_capacity(col_hashed.len() + g.hashed.len() + letters.len() + 32);
                v.extend_from_slice(&col_hashed);
                v.extend_from_slice(&g.hashed);
                let mut seen: [(&str, &str); 4] = [("", ""); 4];
                let mut n_seen = 0;
                for a in last_stem {
                    for bb in &g.last_tokens {
                        if seen[..n_seen].iter().any(|&(x, y)| x == a && y == bb) {
                            continue;
                        }
                        seen[n_seen] = (a, bb);
                        n_seen += 1;
                        buf.clear();
                        buf.extend_from_slice(b"x:");
                        buf.extend_from_slice(a.as_bytes());
                        buf.push(b'|');
                        buf.extend_from_slice(bb.as_bytes());
                        v.push((index_of_bytes(&buf), 1.0));
                    }
                }
                let shared = stem_set.iter().filter(|t| g.tokens.iter().any(|x| x == **t)).count();
                let same_type = f64::from(u8::from(column.ty.as_deref() == g.pk_type));
                let named_key = f64::from(u8::from(cname == g.pk));
                let bare = f64::from(u8::from(stem.is_empty()));
                let values: [f64; 21] = [
                    f64::from(u8::from(stem2 == g.name)),
                    f64::from(u8::from(stem2 == g.singular)),
                    f64::from(u8::from(!stem2.is_empty() && g.name.contains(stem2.as_str()))),
                    f64::from(u8::from(!g.singular.is_empty() && stem2.contains(g.singular.as_str()))),
                    named_key,
                    shared as f64 / 3.0,
                    ends_id,
                    f64::from(u8::from(g.table == ti)),
                    g.columns.min(60) as f64 / 60.0,
                    jaccard_codes(&sg, &g.grams),
                    similarity_in(&stem_chars, &g.singular_chars, &mut prev, &mut cur),
                    prefix(&stem_chars, &g.singular_chars).min(8) as f64 / 8.0,
                    b,
                    f64::from(u8::from(b >= best)),
                    b - max_others,
                    rivals.min(5) as f64 / 5.0,
                    named_key,
                    same_type,
                    named_by[k],
                    id_convention,
                    bare,
                ];
                for (n, &x) in values.iter().enumerate() {
                    if x != 0.0 {
                        v.push((constant(n), x));
                    }
                }
                v.push((key_word, 1.0));
                v.extend_from_slice(&letters);
                buf.clear();
                buf.extend_from_slice(b"p3:");
                buf.extend_from_slice(stem3.as_bytes());
                buf.push(b'|');
                buf.extend_from_slice(g.singular3.as_bytes());
                v.push((index_of_bytes(&buf), 1.0));
                if own {
                    v.push((constant(21), 1.0));
                }
                if g.pseudo {
                    v.push((constant(22), 1.0));
                }
                out.push(Pair { table: ti, column: ci, target: g.table, base: b, same_type, named_key, ends_id, bare,
                                features: row_of(v) });
            }
        }
    }
    out
}

/// graph.py's `graph_features` on dense ids: a column is its table's name and
/// its own name, a target its table's name, as graph.py's dicts key them;
/// and each pair's column, for one target a column.
pub fn graph(s: &Schema, pairs: &[Pair], scores: &[f32]) -> (Vec<[f32; 19]>, Vec<usize>) {
    let mut first_named: HashMap<&str, usize> = HashMap::new();
    let tname: Vec<usize> =
        s.tables.iter().enumerate().map(|(i, t)| *first_named.entry(t.name.as_str()).or_insert(i)).collect();
    let mut ck_of: HashMap<(usize, &str), usize> = HashMap::new();
    let mut ck_names: Vec<(usize, &str)> = Vec::new();
    let ck: Vec<usize> = pairs
        .iter()
        .map(|p| {
            let key = (tname[p.table], s.tables[p.table].columns[p.column].name.as_str());
            *ck_of.entry(key).or_insert_with(|| {
                ck_names.push(key);
                ck_names.len() - 1
            })
        })
        .collect();
    let n = ck_names.len();
    let nt = s.tables.len();
    let mut by_column: Vec<Vec<f32>> = vec![Vec::new(); n];
    let mut back_to: HashMap<(usize, usize), f32> = HashMap::new();
    let mut into_sum = vec![0f32; nt];
    let mut into_confident = vec![0i64; nt];
    let mut into_top: Vec<Vec<f32>> = vec![Vec::new(); nt];
    let mut sibling_top: HashMap<(usize, usize), Vec<f32>> = HashMap::new();
    let keep_two = |v: &mut Vec<f32>, sc: f32| {
        v.push(sc);
        if v.len() > 2 {
            v.sort_by(|a, b| b.partial_cmp(a).unwrap());
            v.truncate(2);
        }
    };
    for (i, p) in pairs.iter().enumerate() {
        let sc = scores[i];
        let (t, tg) = (tname[p.table], tname[p.target]);
        by_column[ck[i]].push(sc);
        let b = back_to.entry((t, tg)).or_insert(0.0);
        if sc > *b {
            *b = sc;
        }
        into_sum[tg] += sc;
        into_confident[tg] += i64::from(sc >= 0.5);
        keep_two(&mut into_top[tg], sc);
        keep_two(sibling_top.entry((t, tg)).or_default(), sc);
    }
    for v in by_column.iter_mut() {
        v.sort_by(|a, b| b.partial_cmp(a).unwrap());
    }
    let confident: Vec<bool> = by_column.iter().map(|v| v[0] >= 0.5).collect();
    let n_confident = confident.iter().filter(|&&c| c).count() as i64;
    let mut confident_in_table = vec![0i64; nt];
    let mut over_half = vec![0usize; n];
    for c in 0..n {
        confident_in_table[ck_names[c].0] += i64::from(confident[c]);
        over_half[c] = by_column[c].iter().filter(|&&x| x >= 0.5).count();
    }
    let mut word_of: HashMap<String, usize> = HashMap::new();
    let word: Vec<usize> = ck_names
        .iter()
        .map(|(_, c)| {
            let w = tokens(c).pop().unwrap_or_default();
            let next = word_of.len();
            *word_of.entry(w).or_insert(next)
        })
        .collect();
    let mut word_sum = vec![0f32; word_of.len()];
    let mut word_count = vec![0i64; word_of.len()];
    for c in 0..n {
        word_sum[word[c]] += by_column[c][0];
        word_count[word[c]] += 1;
    }
    let without = |top: &[f32], sc: f32| -> f32 {
        let (a, b) = match top.len() {
            0 => return 0.0,
            1 => (top[0], None),
            _ => {
                let (x, y) = if top[0] >= top[1] { (top[0], top[1]) } else { (top[1], top[0]) };
                (x, Some(y))
            }
        };
        if a == sc { b.unwrap_or(0.0) } else { a }
    };
    let columns = n.max(1) as f64;
    let mut out = Vec::with_capacity(pairs.len());
    for (i, p) in pairs.iter().enumerate() {
        let sc = scores[i];
        let c = ck[i];
        let (t, tg) = (tname[p.table], tname[p.target]);
        let mine = &by_column[c];
        let rank = 1 + mine.partition_point(|&x| x > sc);
        let next_best = if rank == 1 && mine.len() > 1 { mine[1] } else if rank > 1 { mine[0] } else { 0.0 };
        let w = word[c];
        let mates = word_count[w] - 1;
        let is_confident = i64::from(confident[c]);
        let is_self = p.table == p.target;
        out.push([
            sc,
            rank as f32,
            sc - next_best,
            mine[0],
            over_half[c] as f32,
            into_sum[tg] - sc,
            without(&into_top[tg], sc),
            (into_confident[tg] - i64::from(sc >= 0.5)) as f32,
            without(&sibling_top[&(t, tg)], sc),
            (confident_in_table[t] - is_confident) as f32,
            ((n_confident - is_confident) as f64 / columns) as f32,
            f32::from(u8::from(is_self)),
            if is_self { 0.0 } else { back_to.get(&(tg, t)).copied().unwrap_or(0.0) },
            if mates != 0 { (word_sum[w] - mine[0]) / mates as f32 } else { -1.0 },
            p.base as f32,
            p.same_type as f32,
            p.named_key as f32,
            p.ends_id as f32,
            p.bare as f32,
        ]);
    }
    (out, ck)
}

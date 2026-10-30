//! The same screen as one static page, to look at from a phone or pass on,
//! in the language of the dashbin mockups: the numbers as a band of tiles,
//! then a card a role, each panel a value over its chart — counts by day as
//! bars, shares and times as a line over its area, a split as small charts
//! on one scale, a ranking as a list — drawn by this program, no script, no
//! library, no request, dark or light by the reader's setting.

use crate::doc::{Doc, Number, Panel, Shape, fmt, label, num, once, today, when};
use crate::state::Choices;

const SERIES: [&str; 6] = ["var(--c1)", "var(--c2)", "var(--c3)", "var(--c4)", "var(--c5)", "var(--c6)"];

/// A role's card: its name and what it answers.
fn role(r: &str) -> (&'static str, &'static str) {
    match r {
        "pulse" => ("Pulse", "how much happens"),
        "money" => ("Money", "what is paid"),
        "health" => ("Health", "what fails, and how often"),
        "speed" => ("Speed", "how long things take"),
        "people" => ("People", "who does the work"),
        _ => ("Also", "more the ranker liked"),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn unit(p: &Panel) -> &'static str {
    if p.title.starts_with("share of") { "%" } else { "" }
}

/// A number in full, as a headline reads it: `47,123`, `52.90`.
fn full(x: f64) -> String {
    if x.fract() != 0.0 && x.abs() < 100.0 {
        return format!("{x:.2}");
    }
    let s = format!("{:.0}", x.abs());
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if x < 0.0 { format!("-{out}") } else { out }
}

/// A scale's top and its lines: three or four steps of 1, 2, 2.5 or 5.
fn ticks(hi: f64) -> (f64, Vec<f64>) {
    if hi <= 0.0 {
        return (1.0, vec![0.0, 1.0]);
    }
    let raw = hi / 3.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    let top = (hi / step).ceil() * step;
    let n = (top / step).round() as usize;
    (top, (0..=n).map(|k| k as f64 * step).collect())
}

/// A count by day is a set of buckets, drawn as bars; a share or a time
/// moves between them, drawn as a line.
fn as_bars(p: &Panel) -> bool {
    ["rows of", "sum of", "distinct", "count"].iter().any(|w| p.title.starts_with(w)) || matches!(p.bucket.as_deref(), Some("month" | "year"))
}

fn grid(top: f64, lines: &[f64], u: &str) -> String {
    lines.iter().map(|v| format!("<i class='gl' style='bottom:{:.2}%'><span>{}{u}</span></i>", 100.0 * v / top, esc(&fmt(*v)))).collect()
}

fn axis(times: &[String], bucket: Option<&str>) -> String {
    let n = times.len();
    if n == 0 {
        return String::new();
    }
    let mut picks = vec![0];
    if n > 4 {
        picks.push(n / 2);
    }
    if n > 1 {
        picks.push(n - 1);
    }
    format!("<div class='ax'>{}</div>", picks.iter().map(|&i| format!("<span>{}</span>", esc(&when(&times[i], bucket)))).collect::<String>())
}

/// A line and the area under it, in a box of 1000 by 100 stretched to its
/// place, the stroke kept thin; the last piece dashed while its day goes on.
fn line_svg(vals: &[f64], top: f64, colour: &str, partial: bool, tips: &[String]) -> String {
    let n = vals.len();
    let x = |i: usize| if n < 2 { 500.0 } else { 1000.0 * i as f64 / (n - 1) as f64 };
    let y = |v: f64| 100.0 - 100.0 * (v / top).clamp(0.0, 1.0);
    let upto = if partial && n > 1 { n - 1 } else { n };
    let path = |a: usize, b: usize| (a..b).map(|i| format!("{}{:.1},{:.2}", if i == a { 'M' } else { 'L' }, x(i), y(vals[i]))).collect::<Vec<_>>().join(" ");
    let mut out = format!("<svg viewBox='0 0 1000 100' preserveAspectRatio='none' aria-hidden='true'><path class='ar' style='fill:color-mix(in oklab,{colour} 16%,transparent)' d='{} L{:.1},100 L{:.1},100 Z'/>", path(0, n), x(n - 1), x(0));
    out += &format!("<path class='ln' style='stroke:{colour}' d='{}'/>", path(0, upto));
    if upto < n {
        out += &format!("<path class='ln part' style='stroke:{colour}' d='{}'/>", path(n - 2, n));
    }
    let w = 1000.0 / n.max(1) as f64;
    for (i, tip) in tips.iter().enumerate() {
        out += &format!("<rect x='{:.1}' y='0' width='{w:.1}' height='100' class='hit'><title>{}</title></rect>", x(i) - w / 2.0, esc(tip));
    }
    out + "</svg>"
}

/// A panel over time: its last value, then its chart on a scale from zero.
fn timed(p: &Panel) -> String {
    let times: Vec<String> = p.rows.iter().map(|r| label(&r[0])).collect();
    let vals: Vec<f64> = p.rows.iter().map(|r| num(&r[r.len() - 1]).unwrap_or(0.0)).collect();
    let u = unit(p);
    let b = p.bucket.as_deref();
    let partial = times.last().is_some_and(|t| t.get(..10) == Some(today().as_str()));
    let last = vals[vals.len() - 1];
    let at = if partial { "today, so far".to_string() } else { when(&times[times.len() - 1], b) };
    let hi = vals.iter().copied().fold(0.0f64, f64::max);
    let (top, lines) = ticks(hi);
    let tips: Vec<String> = times.iter().zip(&vals).enumerate().map(|(i, (t, v))| format!("{} · {}{u}{}", when(t, b), full(*v), if partial && i == times.len() - 1 { " so far" } else { "" })).collect();
    let marks = if as_bars(p) {
        let bars: String = vals.iter().zip(&tips).enumerate().map(|(i, (v, tip))| {
            format!("<b class='{}' style='height:{:.2}%'><title>{}</title></b>", if partial && i == vals.len() - 1 { "part" } else { "" }, 100.0 * (v / top).clamp(0.0, 1.0), esc(tip))
        }).collect();
        format!("<div class='bars'>{bars}</div>")
    } else {
        format!("<div class='trace'>{}</div>", line_svg(&vals, top, SERIES[0], partial, &tips))
    };
    format!("<div class='cv'>{}{u}<small>{}</small></div><div class='plot'>{}{marks}</div>{}", esc(&full(last)), esc(&at), grid(top, &lines, u), axis(&times, b))
}

/// A time split by a few values: a small chart a value, one scale for all,
/// its key and its last value over it, each in its own colour.
fn split(p: &Panel) -> String {
    let mut times: Vec<String> = p.rows.iter().map(|r| label(&r[0])).collect();
    times.sort();
    times.dedup();
    let mut groups: Vec<(String, f64)> = Vec::new();
    for r in &p.rows {
        let (g, v) = (label(&r[1]), num(&r[2]).unwrap_or(0.0));
        match groups.iter_mut().find(|x| x.0 == g) {
            Some(x) => x.1 += v,
            None => groups.push((g, v)),
        }
    }
    groups.sort_by(|a, b| b.1.total_cmp(&a.1));
    groups.truncate(SERIES.len());
    let series: Vec<Vec<f64>> = groups.iter().map(|(g, _)| {
        times.iter().map(|t| p.rows.iter().find(|r| &label(&r[0]) == t && &label(&r[1]) == g).and_then(|r| num(&r[2])).unwrap_or(0.0)).collect()
    }).collect();
    let (top, _) = ticks(series.iter().flatten().copied().fold(0.0f64, f64::max));
    let (u, b) = (unit(p), p.bucket.as_deref());
    let partial = times.last().is_some_and(|t| t.get(..10) == Some(today().as_str()));
    let minis: String = groups.iter().zip(&series).enumerate().map(|(k, ((g, _), vals))| {
        let tips: Vec<String> = times.iter().zip(vals).map(|(t, v)| format!("{g}, {} · {}{u}", when(t, b), full(*v))).collect();
        format!("<div class='mini'><div class='mk'><i style='background:{c}'></i><span>{}</span><b>{}{u}</b></div><div class='trace small'>{}</div></div>",
            esc(g), esc(&full(*vals.last().unwrap_or(&0.0))), line_svg(vals, top, SERIES[k], partial, &tips), c = SERIES[k])
    }).collect();
    // no dates under them: one axis under several charts reads as one chart's; the card's head says the window
    format!("<div class='minis'>{minis}</div>")
}

fn ranked(p: &Panel) -> String {
    let rows: String = p.rows.iter().take(8).map(|r| {
        format!("<div class='row'><span class='g'>{}</span><span class='v'>{}</span></div>", esc(&label(&r[0])), esc(&full(num(&r[r.len() - 1]).unwrap_or(0.0))))
    }).collect();
    format!("<div class='list'>{rows}</div>")
}

fn empty(p: &Panel) -> String {
    let (pill, why) = match &p.error {
        Some(e) => ("not run", format!("<div class='meta why'>{}</div>", esc(&e.chars().take(160).collect::<String>()))),
        None => (if p.bucket.is_some() { "no rows in 28 days" } else { "no rows" }, String::new()),
    };
    format!("<div class='cv dim'>No data</div><div class='plot none'><span class='nodata'>{pill}</span></div>{why}")
}

/// How wide a card stands on the page's six columns, before its row is filled.
fn span(p: &Panel) -> usize {
    match p.shape() {
        Shape::Split => 6,
        Shape::Bars | Shape::Number => 2,
        _ => 3,
    }
}

/// Cards to rows of six columns: a row filled card by card, then what is
/// left of it given to its charts, so no row stops short.
fn rows(spans: &[usize], charts: &[bool]) -> Vec<usize> {
    let mut out = spans.to_vec();
    let mut i = 0;
    while i < spans.len() {
        let mut j = i;
        let mut sum = 0;
        while j < spans.len() && sum + spans[j] <= 6 {
            sum += spans[j];
            j += 1;
        }
        let left = 6 - sum;
        let wide: Vec<usize> = (i..j).filter(|&k| charts[k]).collect();
        let take = if wide.is_empty() { (i..j).collect() } else { wide };
        for (n, &k) in take.iter().enumerate() {
            out[k] += left / take.len() + if n < left % take.len() { 1 } else { 0 };
        }
        i = j.max(i + 1);
    }
    out
}

fn card(p: &Panel, kept: bool, span: usize, database: &str) -> String {
    let (name, lede) = role(&p.role);
    let window = match p.bucket.as_deref() {
        Some(b) => format!("last 28 days · by {b}"),
        None => "all rows".into(),
    };
    let body = match p.shape() {
        Shape::Line => timed(p),
        Shape::Split => split(p),
        Shape::Bars => ranked(p),
        Shape::Number => format!("<div class='cv'>{}</div>", esc(&num(&p.rows[0][0]).map(full).unwrap_or_else(|| label(&p.rows[0][0])))),
        Shape::Empty => empty(p),
    };
    let badge = if kept { "<span class='badge'>kept</span>" } else { "" };
    let rows = p.rows.len();
    format!("<section class='sec s{span}'><header class='sh'><h2 class='st'>{name}</h2><span class='meta'>{lede}</span><span class='pv'>{}</span></header>\
<div class='cell'><div class='cl'><span title='{}'>{}</span>{badge}</div>{body}</div>\
<footer class='sf'><span class='tk'><i></i></span>{} · {rows} row{}</footer></section>",
        esc(&window), esc(&p.title), esc(&once(&p.title)), esc(database), if rows == 1 { "" } else { "s" })
}

/// A funnel read from its number: the steps its title names, the counts it shows.
fn steps(n: &Number) -> Option<Vec<(String, f64)>> {
    let names: Vec<&str> = n.title.split(" → ").collect();
    let counts: Vec<f64> = n.shown.split(" → ").map(|s| s.replace(',', "").trim().parse::<f64>()).collect::<Result<_, _>>().ok()?;
    (names.len() == counts.len() && counts.len() >= 2 && counts[0] > 0.0).then(|| names.iter().map(|s| s.to_string()).zip(counts).collect())
}

/// The shares a funnel already shows as its steps' conversions, kept off the band.
fn told_by(steps: &[(String, f64)]) -> Vec<String> {
    let mut out = vec![format!("share of {} {}", steps[0].0, steps[1].0)];
    out.extend(steps.iter().skip(2).map(|(s, _)| format!("share of them {s}")));
    out
}

/// A funnel as bars from one left edge, each the part of the first that is
/// left, on a faint track of the whole, the share that went on between them.
fn funnel(steps: &[(String, f64)], span: usize, database: &str) -> String {
    let first = steps[0].1;
    let mut body = String::new();
    for (i, (name, v)) in steps.iter().enumerate() {
        if i > 0 {
            let prev = steps[i - 1].1;
            let on = if prev > 0.0 { 100.0 * v / prev } else { 0.0 };
            body += &format!("<div class='on'>↓ {on:.1}% go on</div>");
        }
        body += &format!("<div class='step'><div class='fl'><span>{}</span><b>{}</b></div><div class='fb'><i style='width:{:.2}%'></i></div></div>", esc(name), esc(&full(*v)), 100.0 * v / first);
    }
    let end = 100.0 * steps[steps.len() - 1].1 / first;
    format!("<section class='sec s{span}'><header class='sh'><h2 class='st'>Funnel</h2><span class='meta'>how many go on</span><span class='pv'>all rows</span></header>\
<div class='cell'><div class='fun'>{body}</div></div>\
<footer class='sf'><span class='tk'><i></i></span>{} · {end:.1}% of {} reach the last step</footer></section>", esc(database), esc(&steps[0].0))
}

pub fn page(doc: &Doc, panels: &[Panel], c: &Choices) -> String {
    let store = &doc.store;
    let database = if store.database.is_empty() { store.name.as_str() } else { store.database.as_str() };
    let funnels: Vec<Vec<(String, f64)>> = doc.numbers.iter().filter(|n| n.kind == "funnel").filter_map(steps).collect();
    // what a funnel shows stays off the band: its later steps and the shares between them; its first stays a headline
    let mut told: Vec<String> = funnels.iter().flat_map(|f| told_by(f)).collect();
    told.extend(doc.numbers.iter().filter(|n| n.kind == "funnel" && steps(n).is_some()).flat_map(|n| n.parts.iter().skip(1).cloned()));
    let band: Vec<&Number> = doc.numbers.iter().filter(|n| !(n.kind == "funnel" && steps(n).is_some()) && !told.contains(&n.title)).collect();
    let tiles: String = band.iter().map(|n| {
        format!("<div class='cell tile'><div class='cl'>{}</div><div class='cv{}'>{}</div></div>", esc(&n.title), if n.shown.chars().count() > 12 { " long" } else { "" }, esc(&n.shown))
    }).collect();
    // the funnels first, a third of a row each, then the panels; each row filled by its charts
    let mut widths: Vec<usize> = vec![2; funnels.len()];
    widths.extend(panels.iter().map(span));
    let mut charts: Vec<bool> = vec![false; funnels.len()];
    charts.extend(panels.iter().map(|p| matches!(p.shape(), Shape::Line | Shape::Empty)));
    let spans = rows(&widths, &charts);
    let cards: String = funnels.iter().zip(&spans).map(|(f, s)| funnel(f, *s, database))
        .chain(panels.iter().zip(&spans[funnels.len()..]).map(|(p, s)| card(p, c.kept.contains(&p.key), *s, database)))
        .collect();
    let numbers = if doc.numbers.is_empty() {
        String::new()
    } else {
        format!("<section class='sec s6'><header class='sh'><h2 class='st'>Numbers</h2><span class='meta'>read from the schema, counted now</span><span class='pv'>{} · {}</span></header><div class='cells'>{tiles}</div></section>", esc(&store.engine), esc(database))
    };
    format!(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{name} · see</title><style>
:root{{color-scheme:dark;--sans:'Golos Text','Inter','Segoe UI Variable Text','Segoe UI',system-ui,sans-serif;--mono:'JetBrains Mono','Cascadia Mono',ui-monospace,Consolas,monospace;
--bg:#000000;--surface:#0A0A0B;--border:#1F1F23;--border-strong:#2E2E34;--text:#EDEDEF;--text-2:#A8A8B0;--text-3:#85858D;--grid:#17171A;
--c1:#3987e5;--c2:#d95926;--c3:#199e70;--c4:#c98500;--c5:#d55181;--c6:#9085e9}}
@media (prefers-color-scheme: light){{:root{{color-scheme:light;--bg:#F6F6F8;--surface:#FFFFFF;--border:#E3E2E8;--border-strong:#CDCBD4;--text:#1A1920;--text-2:#4C4955;--text-3:#686571;--grid:#EDECF1;
--c1:#2a78d6;--c2:#eb6834;--c3:#1baf7a;--c4:#eda100;--c5:#e87ba4;--c6:#6250d6}}}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--bg);color:var(--text);font:13px/1.45 var(--sans);-webkit-font-smoothing:antialiased}}
main{{max-width:1240px;margin:0 auto;padding:28px 32px 56px}}
.head{{display:flex;align-items:flex-end;gap:16px;flex-wrap:wrap;margin:0 0 18px}}
h1{{font-size:24px;font-weight:600;letter-spacing:-0.02em;margin:0;line-height:1.2}}
.lede{{color:var(--text-2);margin:4px 0 0;max-width:80ch;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden}}
.head .meta{{margin-left:auto}}
.meta{{color:var(--text-3);font-size:12px}}
.page{{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:12px}}
.s2{{grid-column:span 2}} .s3{{grid-column:span 3}} .s4{{grid-column:span 4}} .s5{{grid-column:span 5}} .s6{{grid-column:1/-1}}
.sec{{background:var(--surface);border:1px solid var(--border);border-radius:8px;overflow:hidden;display:flex;flex-direction:column;min-width:0}}
.sh{{display:flex;align-items:baseline;gap:10px;padding:10px 16px;border-bottom:1px solid var(--border);min-width:0}}
.st{{font-size:13px;font-weight:600;margin:0;white-space:nowrap}}
.sh .meta{{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.pv{{font-family:var(--mono);font-size:11px;color:var(--text-3);margin-left:auto;white-space:nowrap}}
.badge{{font-size:10px;font-weight:500;color:var(--text-2);border:1px solid var(--border-strong);border-radius:999px;padding:0 7px;line-height:16px;flex:none}}
.cells{{display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));overflow:hidden}}
.cell{{padding:12px 16px 14px;display:flex;flex-direction:column;min-width:0;flex:1}}
.cells .cell{{box-shadow:1px 0 0 var(--border),0 1px 0 var(--border)}}
.cl{{font-size:12px;color:var(--text-2);display:flex;align-items:center;gap:6px;min-width:0}}
.cl span{{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}
.tile .cl{{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;white-space:normal;overflow-wrap:anywhere}}
.cv.long{{font-size:18px;line-height:1.35}}
.cv{{font-size:24px;font-weight:600;letter-spacing:-0.02em;font-variant-numeric:tabular-nums;margin-top:2px;line-height:1.2}}
.cv small{{font-size:12px;font-weight:500;color:var(--text-3);letter-spacing:0;margin-left:8px}}
.cv.dim{{color:var(--text-3)}}
.plot{{position:relative;height:150px;margin:14px 0 0 38px}}
.gl{{position:absolute;left:0;right:0;border-top:1px solid var(--grid);height:0;font-style:normal}}
.gl span{{position:absolute;right:100%;margin-right:8px;transform:translateY(-50%);font-size:11px;color:var(--text-3);font-variant-numeric:tabular-nums;white-space:nowrap}}
.bars{{position:absolute;inset:0;display:flex;align-items:flex-end;gap:2px}}
.bars b{{flex:1;background:var(--c1);border-radius:2px 2px 0 0;min-width:0;min-height:1px}}
.bars b:hover{{filter:brightness(1.3)}}
.bars b.part{{background:color-mix(in oklab,var(--c1) 45%,transparent)}}
.trace{{position:absolute;inset:0}}
.trace svg{{display:block;width:100%;height:100%;overflow:visible}}
.ln{{fill:none;stroke-width:1.75;vector-effect:non-scaling-stroke;stroke-linejoin:round;stroke-linecap:round}}
.ln.part{{stroke-dasharray:3 3}}
.hit{{fill:transparent}} .hit:hover{{fill:color-mix(in oklab,var(--text) 6%,transparent)}}
.ax{{display:flex;justify-content:space-between;margin-left:38px;padding-top:6px;font-size:11px;color:var(--text-3);font-variant-numeric:tabular-nums}}
.minis{{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:0 16px;margin-top:10px}}
.mini{{min-width:0}}
.mk{{display:flex;align-items:center;gap:8px;font-size:12px;color:var(--text-2)}}
.mk i{{width:10px;height:2px;border-radius:1px;flex:none}}
.mk span{{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}
.mk b{{margin-left:auto;color:var(--text);font-weight:600;font-variant-numeric:tabular-nums}}
.minis .trace{{position:relative;height:64px;margin-top:6px}}
.minis~.ax{{margin-left:0}}
.list{{margin-top:6px}}
.row{{display:flex;align-items:baseline;gap:10px;padding:8px 0;border-top:1px solid var(--border)}}
.row:first-child{{border-top:0}}
.row .g{{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}
.row .v{{margin-left:auto;font-variant-numeric:tabular-nums}}
.plot.none{{border-top:1px dashed var(--border-strong);height:0;margin:56px 0 56px}}
.nodata{{position:absolute;left:50%;top:0;transform:translate(-50%,-50%);font-size:11px;color:var(--text-2);background:var(--surface);border:1px solid var(--border-strong);border-radius:999px;padding:1px 9px;white-space:nowrap}}
.why{{margin-top:-36px}}
.sf{{display:flex;align-items:center;gap:8px;padding:7px 16px;border-top:1px solid var(--border);font-size:11px;color:var(--text-3);margin-top:auto}}
.tk{{display:inline-flex}} .tk i{{width:5px;height:10px;border-radius:1px;background:var(--text-3)}}
.fun{{margin-top:2px}}
.fl{{display:flex;align-items:baseline;gap:10px;font-size:12px;color:var(--text-2)}}
.fl span{{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}
.fl b{{margin-left:auto;font-size:15px;font-weight:600;color:var(--text);font-variant-numeric:tabular-nums}}
.fb{{height:18px;margin-top:4px;border-radius:2px;background:color-mix(in oklab,var(--c1) 12%,transparent)}}
.fb i{{display:block;height:100%;border-radius:2px;background:var(--c1)}}
.on{{font-size:11px;color:var(--text-3);padding:6px 0;font-variant-numeric:tabular-nums}}
.note{{color:var(--text-3);font-size:12px;margin-top:18px}}
@media (max-width:900px){{.page>*{{grid-column:1/-1}}}}
@media (max-width:760px){{main{{padding:16px 16px 32px}} .head .meta{{margin-left:0}} .cells{{grid-template-columns:repeat(2,minmax(0,1fr))}} .pv{{display:none}}}}
</style></head><body><main>
<div class="head"><div><h1>{name}</h1><p class="lede">{about}</p></div><span class="meta">{engine} · {nn} numbers · {np} panels · guessed from the schema</span></div>
<div class="page">{numbers}{cards}</div>
<p class="note">Drawn by the see prototype. Every panel is guessed from the schema; every number is read from the database.</p>
</main></body></html>"#,
        name = esc(&store.name), about = esc(&store.about), engine = esc(&store.engine), nn = doc.numbers.len(), np = panels.len())
}

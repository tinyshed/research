//! The same screen as one static page, to look at from a phone or pass on:
//! the numbers as tiles, each panel drawn as inline SVG by this program — a
//! line, a split as a line a value with its key, bars — no script, no
//! library, light and dark by the reader's setting.

use crate::doc::{Doc, Panel, Shape, fmt, label, num};
use crate::state::Choices;

const COLOURS: [&str; 6] = ["var(--s1)", "var(--s2)", "var(--s3)", "var(--s4)", "var(--s5)", "var(--s6)"];

fn role_colour(role: &str) -> &'static str {
    match role {
        "pulse" => "var(--s1)",
        "money" => "var(--s3)",
        "health" => "var(--s4)",
        "speed" => "var(--s2)",
        "people" => "var(--s5)",
        _ => "var(--mut)",
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn unit(p: &Panel) -> &'static str {
    if p.title.starts_with("share of") { "%" } else { "" }
}

fn lines(series: &[(String, Vec<f64>)], times: &[String], u: &str, title: &str) -> String {
    let (w, h, l, r, t, b) = (320.0, 132.0, 40.0, 8.0, 8.0, 18.0);
    let hi = series.iter().flat_map(|s| s.1.iter().copied()).fold(0.0f64, f64::max).max(1e-9);
    let lo = series.iter().flat_map(|s| s.1.iter().copied()).fold(0.0f64, f64::min);
    let n = times.len().max(2) - 1;
    let x = |i: usize| l + (w - l - r) * i as f64 / n as f64;
    let y = |v: f64| t + (h - t - b) * (1.0 - (v - lo) / (hi - lo).max(1e-9));
    let mut out = format!("<svg viewBox='0 0 {w} {h}' role='img' aria-label='{}'>", esc(title));
    out += &format!("<line x1='{l}' x2='{}' y1='{:.1}' y2='{:.1}' class='axis'/>", w - r, y(0.0f64.max(lo)), y(0.0f64.max(lo)));
    out += &format!("<text x='{}' y='{}' class='tick' text-anchor='end'>{}{u}</text>", l - 4.0, t + 8.0, esc(&fmt(hi)));
    out += &format!("<text x='{}' y='{:.1}' class='tick' text-anchor='end'>{}{u}</text>", l - 4.0, y(lo), esc(&fmt(lo)));
    for (k, (name, vals)) in series.iter().enumerate() {
        let d: Vec<String> = vals.iter().enumerate().map(|(i, v)| format!("{}{:.1},{:.1}", if i == 0 { 'M' } else { 'L' }, x(i), y(*v))).collect();
        out += &format!("<path d='{}' class='line' style='stroke:{}'/>", d.join(" "), COLOURS[k % 6]);
        for (i, v) in vals.iter().enumerate() {
            out += &format!("<circle cx='{:.1}' cy='{:.1}' r='6' fill='transparent'><title>{}{}: {}{u}</title></circle>",
                x(i), y(*v), if series.len() > 1 { format!("{}, ", esc(name)) } else { String::new() }, esc(&times[i.min(times.len() - 1)]), esc(&fmt(*v)));
        }
    }
    if let (Some(a), Some(z)) = (times.first(), times.last()) {
        out += &format!("<text x='{l}' y='{}' class='tick'>{}</text><text x='{}' y='{}' class='tick' text-anchor='end'>{}</text>", h - 4.0, esc(a), w - r, h - 4.0, esc(z));
    }
    out += "</svg>";
    if series.len() > 1 {
        out += "<div class='key'>";
        for (k, (name, _)) in series.iter().enumerate() {
            out += &format!("<span><i style='background:{}'></i>{}</span>", COLOURS[k % 6], esc(name));
        }
        out += "</div>";
    }
    out
}

fn chart(p: &Panel) -> String {
    match p.shape() {
        Shape::Empty => format!("<div class='muted'>{}</div>", esc(&p.error.clone().map(|e| format!("not run: {e}")).unwrap_or("no rows".into()))),
        Shape::Number => format!("<div class='hero'>{}</div>", esc(&num(&p.rows[0][0]).map(fmt).unwrap_or_else(|| label(&p.rows[0][0])))),
        Shape::Line => {
            let times: Vec<String> = p.rows.iter().map(|r| label(&r[0])).collect();
            let vals: Vec<f64> = p.rows.iter().map(|r| num(&r[r.len() - 1]).unwrap_or(0.0)).collect();
            lines(&[(String::new(), vals)], &times, unit(p), &p.title)
        }
        Shape::Split => {
            let mut times: Vec<String> = p.rows.iter().map(|r| label(&r[0])).collect();
            times.sort();
            times.dedup();
            let mut groups: Vec<(String, f64)> = Vec::new();
            for r in &p.rows {
                let g = label(&r[1]);
                let v = num(&r[2]).unwrap_or(0.0);
                match groups.iter_mut().find(|x| x.0 == g) {
                    Some(x) => x.1 += v,
                    None => groups.push((g, v)),
                }
            }
            groups.sort_by(|a, b| b.1.total_cmp(&a.1));
            groups.truncate(6);
            let series: Vec<(String, Vec<f64>)> = groups.iter().map(|(g, _)| {
                (g.clone(), times.iter().map(|t| p.rows.iter().find(|r| &label(&r[0]) == t && &label(&r[1]) == g).and_then(|r| num(&r[2])).unwrap_or(0.0)).collect())
            }).collect();
            lines(&series, &times, unit(p), &p.title)
        }
        Shape::Bars => {
            let rows: Vec<(String, f64)> = p.rows.iter().take(12).map(|r| (label(&r[0]), num(&r[r.len() - 1]).unwrap_or(0.0))).collect();
            let hi = rows.iter().map(|x| x.1).fold(0.0f64, f64::max).max(1e-9);
            let mut out = String::from("<div class='bars'>");
            for (g, v) in rows {
                out += &format!("<div class='bar'><span class='g'>{}</span><span class='b' style='width:{:.1}%'><title>{}</title></span><span class='v'>{}</span></div>",
                    esc(&g), 100.0 * v.max(0.0) / hi, esc(&fmt(v)), esc(&fmt(v)));
            }
            out + "</div>"
        }
    }
}

pub fn page(doc: &Doc, panels: &[Panel], c: &Choices) -> String {
    let tiles: String = doc.numbers.iter().map(|n| format!("<div class='tile'><div class='k'>{}</div><div class='n'>{}</div></div>", esc(&n.title), esc(&n.shown))).collect();
    let cards: String = panels.iter().map(|p| {
        let kept = if c.kept.contains(&p.key) { "<span class='kept'>kept</span>" } else { "" };
        format!("<section class='card'><header><span class='role' style='background:{}' title='{}'></span><h2>{}</h2>{kept}</header>{}</section>", role_colour(&p.role), esc(&p.role), esc(&p.title), chart(p))
    }).collect();
    format!(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>see · {name}</title><style>
:root{{--bg:#eff1f5;--card:#e6e9ef;--fg:#4c4f69;--mut:#8c8fa1;--line:#ccd0da;--s1:#1e66f5;--s2:#fe640b;--s3:#40a02b;--s4:#d20f39;--s5:#8839ef;--s6:#179299;--n:#4c4f69;--sel:#7287fd}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--bg:#1e1e2e;--card:#181825;--fg:#cdd6f4;--mut:#6c7086;--line:#313244;--s1:#89b4fa;--s2:#fab387;--s3:#a6e3a1;--s4:#f38ba8;--s5:#cba6f7;--s6:#94e2d5;--n:#cdd6f4;--sel:#b4befe}}}}
*{{box-sizing:border-box}} body{{margin:0;padding:24px 16px;background:var(--bg);color:var(--fg);font:14px/1.5 "Cascadia Mono","JetBrains Mono",ui-monospace,Menlo,Consolas,monospace;max-width:1200px;margin-inline:auto}}
h1{{font-size:18px;margin:0 0 2px}} .about{{color:var(--mut);margin:0 0 14px;max-width:70ch}}
.tiles{{display:grid;grid-template-columns:repeat(auto-fill,minmax(170px,1fr));gap:6px 24px;padding-bottom:16px;margin-bottom:18px;border-bottom:1px solid var(--line)}}
.tile{{padding:6px 0}}
.tile .k{{font-size:12px;color:var(--mut)}} .tile .n{{font-size:22px;font-weight:650;color:var(--n);font-variant-numeric:tabular-nums}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,420px),1fr));gap:28px 36px}}
.card{{min-width:0}}
.card h2{{display:inline;font-size:13px;font-weight:500;margin:0;overflow-wrap:anywhere}} .card header{{margin-bottom:8px}} .role{{display:inline-block;width:8px;height:8px;border-radius:50%;margin-right:8px;vertical-align:1px}}
.kept{{font-size:11px;color:var(--s3);margin-left:8px}}
svg{{width:100%;height:auto;display:block}} .line{{fill:none;stroke-width:2;stroke-linejoin:round;stroke-linecap:round}}
.axis{{stroke:var(--line)}} .tick{{fill:var(--mut);font-size:10px}}
.key{{display:flex;flex-wrap:wrap;gap:10px;font-size:12px;color:var(--mut);margin-top:4px}} .key i{{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:4px;vertical-align:-1px}}
.bars{{display:grid;gap:3px}} .bar{{display:grid;grid-template-columns:minmax(0,9em) 1fr auto;gap:6px;align-items:center;font-size:12px}}
.bar .g{{color:var(--mut);text-align:right;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}} .bar .b{{height:12px;background:var(--s1);border-radius:3px;min-width:2px}}
.bar .v{{font-variant-numeric:tabular-nums}} .hero{{font-size:40px;font-weight:650}} .muted{{color:var(--mut)}}
footer{{color:var(--mut);font-size:12px;margin-top:16px}}
</style></head><body>
<h1>{name}</h1><p class="about">{about}</p>
<div class="tiles">{tiles}</div>
<div class="cards">{cards}</div>
<footer>Drawn by the see prototype. Every panel is guessed from the schema; the numbers are read from the database.</footer>
</body></html>"#, name = esc(&doc.store.name), about = esc(&doc.store.about))
}

//! The screen: the store, its row of numbers, its panels in two columns, and
//! the keys of chapter 11 — a panel alone, keep, drop, add, undo, save.
//! Drawn quietly: one palette, no boxes, colour only where it means
//! something, a panel told from its neighbours by space, not by lines.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph};

use crate::doc::{Doc, Panel, Shape, fmt, label, num};
use crate::state::{self, State};

/// The terminal's own colours: its background and text untouched, its
/// grey for what is secondary, its bright colours where a colour means
/// something — so the screen takes whatever theme the terminal has.
pub mod pal {
    use ratatui::style::Color;
    pub const QUIET: Color = Color::DarkGray;
    pub const TEXT: Color = Color::Reset;
    pub const SUBTEXT: Color = Color::Reset;
    pub const OVERLAY0: Color = Color::DarkGray;
    pub const SURFACE0: Color = Color::DarkGray;
    pub const SURFACE1: Color = Color::DarkGray;
    pub const LAVENDER: Color = Color::Cyan;
    pub const BLUE: Color = Color::LightBlue;
    pub const GREEN: Color = Color::LightGreen;
    pub const PEACH: Color = Color::LightYellow;
    pub const RED: Color = Color::LightRed;
    pub const MAUVE: Color = Color::LightMagenta;
    pub const TEAL: Color = Color::LightCyan;
    pub const YELLOW: Color = Color::Yellow;
}
use pal::*;

pub const SERIES: [Color; 6] = [BLUE, PEACH, GREEN, MAUVE, TEAL, YELLOW];

fn role_colour(role: &str) -> Color {
    match role {
        "pulse" => BLUE,
        "money" => GREEN,
        "health" => RED,
        "speed" => PEACH,
        "people" => MAUVE,
        _ => OVERLAY0,
    }
}

#[derive(PartialEq, Eq)]
pub enum Mode {
    Grid,
    Full,
    Add,
}

pub struct App {
    pub doc: Doc,
    pub state: State,
    pub panels: Vec<Panel>,
    pub at: usize,
    pub mode: Mode,
    pub pick: ListState,
    pub note: String,
    pub done: bool,
    /// the grid as last drawn: its columns, and the first row in view
    pub cols: usize,
    pub top: usize,
}

impl App {
    pub fn new(doc: Doc, state: State) -> App {
        let panels = state::screen(&doc, &state.choices());
        let mut pick = ListState::default();
        pick.select(Some(0));
        App { doc, state, panels, at: 0, mode: Mode::Grid, pick, note: String::new(), done: false, cols: 2, top: 0 }
    }

    fn refresh(&mut self) {
        self.panels = state::screen(&self.doc, &self.state.choices());
        if self.at >= self.panels.len() {
            self.at = self.panels.len().saturating_sub(1);
        }
    }

    /// The spares the add list offers: every role's, minus what is shown.
    pub fn offers(&self) -> Vec<(String, Panel)> {
        let shown: Vec<&str> = self.panels.iter().map(|p| p.key.as_str()).collect();
        self.doc
            .spares
            .iter()
            .flat_map(|(r, v)| v.iter().map(move |p| (r.clone(), p.clone())))
            .filter(|(_, p)| !shown.contains(&p.key.as_str()))
            .collect()
    }

    pub fn key(&mut self, k: KeyCode) {
        if self.mode == Mode::Add {
            let n = self.offers().len();
            match k {
                KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Grid,
                KeyCode::Down | KeyCode::Char('j') => self.pick.select(Some((self.pick.selected().unwrap_or(0) + 1).min(n.saturating_sub(1)))),
                KeyCode::Up | KeyCode::Char('k') => self.pick.select(Some(self.pick.selected().unwrap_or(0).saturating_sub(1))),
                KeyCode::Enter => {
                    if let Some((_, p)) = self.offers().get(self.pick.selected().unwrap_or(0)).cloned() {
                        self.state.add(&p.key);
                        self.note = format!("added  {}", p.title);
                        self.refresh();
                        self.at = self.panels.len().saturating_sub(1);
                    }
                    self.mode = Mode::Grid;
                }
                _ => {}
            }
            return;
        }
        let n = self.panels.len();
        match k {
            KeyCode::Char('q') => self.done = true,
            KeyCode::Esc => {
                if self.mode == Mode::Full { self.mode = Mode::Grid } else { self.done = true }
            }
            KeyCode::Right | KeyCode::Char('l') => self.at = (self.at + 1).min(n.saturating_sub(1)),
            KeyCode::Left | KeyCode::Char('h') => self.at = self.at.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.at = (self.at + self.cols).min(n.saturating_sub(1)),
            KeyCode::Up | KeyCode::Char('k') => self.at = self.at.saturating_sub(self.cols),
            KeyCode::Enter => self.mode = if self.mode == Mode::Full { Mode::Grid } else { Mode::Full },
            KeyCode::Char('f') => {
                if let Some(p) = self.panels.get(self.at) {
                    let kept = self.state.choices().kept.contains(&p.key);
                    self.state.keep(&p.key.clone());
                    self.note = format!("{}  {}", if kept { "unkept" } else { "kept" }, p.title);
                }
            }
            KeyCode::Char('x') => {
                if let Some(p) = self.panels.get(self.at).cloned() {
                    self.state.drop_panel(&p.key);
                    self.refresh();
                    let next = self.panels.get(self.at).map(|q| q.title.clone()).unwrap_or_default();
                    self.note = format!("dropped  {}  →  {}", p.title, next);
                }
            }
            KeyCode::Char('+') | KeyCode::Char('a') => {
                self.pick.select(Some(0));
                self.mode = Mode::Add;
            }
            KeyCode::Char('u') => {
                self.note = if self.state.undo() { "undone".into() } else { "nothing to undo".into() };
                self.refresh();
            }
            KeyCode::Char('s') => self.note = save(self),
            _ => {}
        }
    }
}

/// The kept panels, or all shown when none is kept, as a dashboard's file.
fn save(app: &App) -> String {
    let c = app.state.choices();
    let chosen: Vec<&Panel> = if c.kept.is_empty() { app.panels.iter().collect() } else { app.panels.iter().filter(|p| c.kept.contains(&p.key)).collect() };
    let mut s = format!("# a dashboard saved by see from {}\nname = \"{}\"\n", app.doc.store.name, app.doc.store.name);
    for p in &chosen {
        s += &format!("\n[[panel]]\ntitle = {:?}\nrole = {:?}\nguessed = true\n", p.title, p.role);
    }
    let path = app.state.path().with_file_name(format!("{}.dash.toml", app.doc.store.name));
    match std::fs::write(&path, s) {
        Ok(()) => format!("saved {} panels  {}", chosen.len(), path.display()),
        Err(e) => format!("not saved  {e}"),
    }
}

/// Words to lines of a width, at most so many lines, the last cut short.
fn wrap(s: &str, width: usize, most: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for w in s.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + w.chars().count() <= width => {
                l.push(' ');
                l.push_str(w);
            }
            _ => lines.push(w.to_string()),
        }
    }
    let most = most.max(1);
    if lines.len() > most {
        let rest = lines.split_off(most - 1).join(" ");
        lines.push(rest);
    }
    lines.iter().map(|l| cut(l, width)).collect()
}

/// Words that only join, never left at the end of a cut line.
const JOINERS: [&str; 9] = ["by", "of", "in", "with", "split", "a", "the", "and", "per"];

/// A line cut to a width at a word, `…` where it was cut.
fn cut(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let room = width.saturating_sub(1);
    let (mut words, mut len) = (Vec::new(), 0);
    for w in s.split_whitespace() {
        let more = w.chars().count() + if words.is_empty() { 0 } else { 1 };
        if len + more > room {
            break;
        }
        len += more;
        words.push(w);
    }
    while words.len() > 1 && JOINERS.contains(&words[words.len() - 1]) {
        words.pop();
    }
    // at a word unless that throws away most of the room, as a long single name would
    let joined = words.join(" ");
    let head: String = if joined.chars().count() * 2 < room { s.chars().take(room).collect() } else { joined };
    format!("{}…", head.trim_end_matches([',', ' ']))
}

/// A panel's title for a width: each table named once, so `loan.date_loan`
/// after `loan` reads `date_loan`; if that is still too long, the columns
/// without their tables; then cut at a word. Nothing scrolls: a title that
/// moves pulls the eye from the charts all the time.
fn fit_title(title: &str, width: usize) -> String {
    let dotted = |w: &str| {
        w.split_once('.')
            .filter(|(t, c)| !c.is_empty() && t.chars().next().is_some_and(|ch| ch.is_alphabetic() || ch == '_') && t.chars().all(|ch| ch.is_alphanumeric() || ch == '_'))
            .map(|(t, c)| (t.to_string(), c.to_string()))
    };
    let mut seen: Vec<String> = Vec::new();
    let once = title
        .split_whitespace()
        .map(|w| match dotted(w) {
            Some((t, c)) if seen.contains(&t) => c,
            Some((t, _)) => {
                seen.push(t);
                w.to_string()
            }
            None => {
                seen.push(w.trim_end_matches(',').to_string());
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if once.chars().count() <= width {
        return once;
    }
    let bare = once.split_whitespace().map(|w| dotted(w).map(|(_, c)| c).unwrap_or_else(|| w.to_string())).collect::<Vec<_>>().join(" ");
    cut(&bare, width)
}

/// Today as the database writes a day, in UTC: the day a chart's last point
/// may only have begun.
fn today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // days to a civil date, after Howard Hinnant
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// The last value of a line, written where it ends, ` today` after it when
/// the last day is still going.
fn end_tag(v: f64, unit: &str, last_time: Option<&String>) -> (String, bool) {
    (format!("{}{unit}", fmt(v)), last_time.is_some_and(|t| t.get(..10) == Some(today().as_str())))
}

fn put_tag(f: &mut Frame, x: u16, y: u16, (value, partial): &(String, bool), colour: Color) {
    let mut spans = vec![Span::styled(value.clone(), Style::default().fg(colour))];
    if *partial {
        spans.push(dim(" today"));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), Rect { x, y, width: tag_width(&(value.clone(), *partial)), height: 1 });
}

fn tag_width((value, partial): &(String, bool)) -> u16 {
    value.chars().count() as u16 + if *partial { 6 } else { 0 }
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(OVERLAY0))
}

/// How the screen fits a terminal: the numbers in one or two rows, the
/// panels in one to three columns, each at least `MIN_PANEL` lines high, and
/// what does not fit scrolled to as the selection moves.
const MIN_PANEL: u16 = 8;
const LOW: u16 = 34;

pub fn draw(f: &mut Frame, app: &mut App) {
    let full = f.area();
    let area = Rect { x: full.x + 2, y: full.y + 1, width: full.width.saturating_sub(4), height: full.height.saturating_sub(2) };
    let per = (area.width / 20).clamp(1, 8) as usize;
    let number_rows = if area.height < LOW { 1 } else { 2 };
    let shown_numbers = app.doc.numbers.len().min(per * number_rows);
    let num_rows = shown_numbers.div_ceil(per) as u16;
    // a second line for what a number counts, when one of them needs it
    let tile = (area.width as usize / per).saturating_sub(2);
    let label_lines: u16 = if app.doc.numbers.iter().take(shown_numbers).any(|n| n.title.chars().count() > tile) { 2 } else { 1 };
    let row_h = label_lines + 2;
    let [head, _, nums, rule, body, _, foot] = Layout::vertical([
        Constraint::Length(if area.height < LOW { 1 } else { 2 }),
        Constraint::Length(1),
        Constraint::Length(num_rows * row_h),
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let store = &app.doc.store;
    let mut head_lines = vec![Line::from(vec![
        Span::styled(store.name.clone(), Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        dim(format!("   {}  ·  {} numbers  ·  {} panels  ·  guessed from the schema", store.engine, app.doc.numbers.len(), app.panels.len())),
    ])];
    if head.height > 1 {
        head_lines.push(Line::styled(store.about.chars().take(area.width as usize).collect::<String>(), Style::default().fg(SUBTEXT)));
    }
    f.render_widget(Paragraph::new(head_lines), head);

    // the numbers: a value, and under it what it counts, on a line or two
    let rows = Layout::vertical(vec![Constraint::Length(row_h); num_rows as usize]).split(nums);
    for (r, rect) in rows.iter().enumerate() {
        let cells = Layout::horizontal(vec![Constraint::Ratio(1, per as u32); per]).split(*rect);
        for (c, cell) in cells.iter().enumerate() {
            if let Some(n) = app.doc.numbers.get(r * per + c) {
                let w = cell.width.saturating_sub(2) as usize;
                let colour = TEXT;
                let mut lines = vec![Line::styled(n.shown.clone(), Style::default().fg(colour).add_modifier(Modifier::BOLD))];
                lines.extend(wrap(&n.title, w, label_lines as usize).into_iter().map(|l| Line::styled(l, Style::default().fg(OVERLAY0))));
                f.render_widget(Paragraph::new(lines), *cell);
            }
        }
    }
    let more = app.doc.numbers.len().saturating_sub(shown_numbers);
    let mut rule_line = "─".repeat(area.width as usize);
    if more > 0 {
        let tag = format!(" {more} more numbers in --html ");
        rule_line = format!("{}{tag}", "─".repeat((area.width as usize).saturating_sub(tag.chars().count())));
    }
    f.render_widget(Paragraph::new(Line::styled(rule_line, Style::default().fg(SURFACE0))), rule);

    // the panels
    let kept = app.state.choices().kept;
    let n = app.panels.len();
    if app.mode == Mode::Full {
        if let Some(p) = app.panels.get(app.at).cloned() {
            panel(f, body, &p, true, kept.contains(&p.key));
        }
    } else if n > 0 {
        let cols = if body.width >= 168 { 3 } else if body.width >= 96 { 2 } else { 1 };
        app.cols = cols;
        let lines_total = n.div_ceil(cols);
        let fit = ((body.height + 1) / (MIN_PANEL + 1)).max(1) as usize;
        let rows_shown = lines_total.min(fit);
        let sel = app.at / cols;
        if sel < app.top {
            app.top = sel;
        }
        if sel >= app.top + rows_shown {
            app.top = sel + 1 - rows_shown;
        }
        app.top = app.top.min(lines_total.saturating_sub(rows_shown));
        let rows = Layout::vertical(vec![Constraint::Ratio(1, rows_shown as u32); rows_shown]).spacing(1).split(body);
        for (r, rect) in rows.iter().enumerate() {
            let line = app.top + r;
            let parts = Layout::horizontal(vec![Constraint::Ratio(1, cols as u32); cols]).spacing(4).split(*rect);
            for c in 0..cols {
                let i = line * cols + c;
                if let Some(p) = app.panels.get(i).cloned() {
                    panel(f, parts[c], &p, i == app.at, kept.contains(&p.key));
                }
            }
        }
        let above = app.top * cols;
        let below = n.saturating_sub((app.top + rows_shown) * cols);
        if below > 0 || above > 0 {
            let tag = format!("{}{}", if above > 0 { format!("↑ {above} above  ") } else { String::new() }, if below > 0 { format!("↓ {below} below") } else { String::new() });
            let w = tag.chars().count() as u16;
            f.render_widget(Paragraph::new(Line::styled(tag, Style::default().fg(LAVENDER))), Rect { x: body.x + body.width.saturating_sub(w), y: foot.y, width: w, height: 1 });
        }
    }

    // the keys, quietly, and what the last one did
    let mut spans = Vec::new();
    for (k, what) in [("←↑↓→", "move"), ("enter", "alone"), ("f", "keep"), ("x", "drop"), ("+", "add"), ("u", "undo"), ("s", "save"), ("q", "quit")] {
        spans.push(Span::styled(k, Style::default().add_modifier(Modifier::BOLD)));
        spans.push(dim(format!(" {what}   ")));
    }
    if !app.note.is_empty() {
        spans.push(Span::styled(format!(" {}", app.note), Style::default().fg(SUBTEXT)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), foot);

    if app.mode == Mode::Add {
        add_list(f, app, body);
    }
}

fn panel(f: &mut Frame, area: Rect, p: &Panel, selected: bool, kept: bool) {
    if area.height < 3 || area.width < 12 {
        return;
    }
    let [title, plot] = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).areas(area);
    if selected {
        for y in area.y..area.y + area.height {
            f.buffer_mut()[(area.x, y)].set_symbol("▎").set_fg(LAVENDER);
        }
    }
    let inner = |r: Rect| Rect { x: r.x + 2, width: r.width.saturating_sub(2), ..r };
    let room = title.width.saturating_sub(if kept { 10 } else { 4 }) as usize;
    let mut spans = vec![
        Span::styled("● ", Style::default().fg(role_colour(&p.role))),
        Span::styled(fit_title(&p.title, room), if selected { Style::default().fg(TEXT).add_modifier(Modifier::BOLD) } else { Style::default().fg(SUBTEXT) }),
    ];
    if kept {
        spans.push(Span::styled("  kept", Style::default().fg(GREEN)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), inner(title));
    let a = inner(plot);
    let colour = role_colour(&p.role).max_blue();
    match p.shape() {
        Shape::Line => {
            let times: Vec<String> = p.rows.iter().map(|r| label(&r[0])).collect();
            let vals: Vec<f64> = p.rows.iter().map(|r| num(&r[r.len() - 1]).unwrap_or(0.0)).collect();
            chart(f, a, &vals, &times, colour, unit(p));
        }
        Shape::Split => split(f, a, p),
        Shape::Bars => bars(f, a, p),
        Shape::Number => {
            let v = num(&p.rows[0][0]).map(fmt).unwrap_or_else(|| label(&p.rows[0][0]));
            f.render_widget(Paragraph::new(Line::styled(v, Style::default().fg(TEXT).add_modifier(Modifier::BOLD))), a);
        }
        Shape::Empty => {
            let why = p.error.clone().map(|e| format!("not run: {e}")).unwrap_or_else(|| "no rows".into());
            f.render_widget(Paragraph::new(dim(why)), a);
        }
    }
}

fn unit(p: &Panel) -> &'static str {
    if p.title.starts_with("share of") { "%" } else { "" }
}

fn short_date(s: &str) -> String {
    const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    match (s.get(5..7).and_then(|m| m.parse::<usize>().ok()), s.get(8..10).and_then(|d| d.parse::<u32>().ok())) {
        (Some(m), Some(d)) if (1..=12).contains(&m) && s.len() >= 10 => format!("{} {}", M[m - 1], d),
        _ => s.to_string(),
    }
}

trait Tint {
    fn max_blue(self) -> Color;
}

impl Tint for Color {
    /// A role's colour for its chart, the quiet ones drawn in blue.
    fn max_blue(self) -> Color {
        if self == OVERLAY0 { BLUE } else { self }
    }
}

/// A series as one thin line, as asciichart draws it: a row a level, the
/// steps between points in box lines, nothing filled, so the shape is what
/// is read. Returns the row the line ends on.
fn trace(f: &mut Frame, plot: Rect, vals: &[f64], lo: f64, hi: f64, colour: Color) -> u16 {
    let top = plot.height.saturating_sub(1);
    let row = |l: u16| plot.y + top - l;
    let n = vals.len();
    if n == 0 || plot.width == 0 || plot.height == 0 {
        return row(0);
    }
    let level = |v: f64| (((v - lo) / (hi - lo)) * top as f64).round().clamp(0.0, top as f64) as u16;
    let mut prev = None;
    for x in 0..plot.width {
        let i = ((x as usize * n) / plot.width as usize).min(n - 1);
        let l = level(vals[i]);
        let p = prev.unwrap_or(l);
        let mut put = |y: u16, s: &str| {
            f.buffer_mut()[(plot.x + x, y)].set_symbol(s).set_fg(colour);
        };
        if l == p {
            put(row(l), "─");
        } else {
            for k in p.min(l) + 1..p.max(l) {
                put(row(k), "│");
            }
            if l > p {
                put(row(p), "╯");
                put(row(l), "╭");
            } else {
                put(row(p), "╮");
                put(row(l), "╰");
            }
        }
        prev = Some(l);
    }
    row(prev.unwrap_or(0))
}

/// A series over time: its highest and lowest value at the left, the first
/// and last day under it, the line between.
fn chart(f: &mut Frame, area: Rect, vals: &[f64], times: &[String], colour: Color, unit: &str) {
    if vals.is_empty() || area.height < 2 {
        return;
    }
    // a line, unlike a bar, needs no floor at zero: its own lowest to highest, both written at the left
    let lo = vals.iter().copied().fold(f64::MAX, f64::min);
    let hi = vals.iter().copied().fold(f64::MIN, f64::max).max(lo + 1e-9);
    let labels = [format!("{}{unit}", fmt(hi)), format!("{}{unit}", fmt(lo))];
    let lw = labels.iter().map(|l| l.chars().count()).max().unwrap_or(1) as u16 + 1;
    let dates = area.height >= 4 && !times.is_empty();
    let ph = area.height - if dates { 1 } else { 0 };
    let tag = end_tag(vals[vals.len() - 1], unit, times.last());
    let plot = Rect { x: area.x + lw, y: area.y, width: area.width.saturating_sub(lw + tag_width(&tag) + 1), height: ph };
    if plot.width == 0 {
        return;
    }
    f.buffer_mut().set_string(area.x, area.y, &labels[0], Style::default().fg(OVERLAY0));
    if ph > 1 {
        f.buffer_mut().set_string(area.x, area.y + ph - 1, &labels[1], Style::default().fg(OVERLAY0));
    }
    let end = trace(f, plot, vals, lo, hi, colour);
    put_tag(f, plot.x + plot.width + 1, end, &tag, colour);
    if dates {
        let (a, z) = (short_date(&times[0]), short_date(&times[times.len() - 1]));
        f.buffer_mut().set_string(plot.x, area.y + ph, &a, Style::default().fg(OVERLAY0));
        let zw = z.chars().count() as u16;
        if plot.width > zw + a.chars().count() as u16 + 2 {
            f.buffer_mut().set_string(plot.x + plot.width - zw, area.y + ph, &z, Style::default().fg(OVERLAY0));
        }
    }
}

/// A time split by a few values: a band a value, its line on one scale,
/// its key at its left and its last value where the line ends, each in its
/// own colour.
fn split(f: &mut Frame, area: Rect, p: &Panel) {
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
    let dates = area.height >= 4;
    let room = area.height.saturating_sub(if dates { 1 } else { 0 }) as usize;
    // three rows a band at least, or the line has no levels to move between
    groups.truncate(SERIES.len().min((room / 3).max(1)));
    let k = groups.len().max(1);
    let each = (room / k).max(1) as u16;
    let key_w = groups.iter().map(|g| g.0.chars().count()).max().unwrap_or(4).min(14) as u16 + 2;
    let series: Vec<Vec<f64>> = groups.iter().map(|(g, _)| {
        times.iter().map(|t| p.rows.iter().find(|r| &label(&r[0]) == t && &label(&r[1]) == g).and_then(|r| num(&r[2])).unwrap_or(0.0)).collect()
    }).collect();
    let hi = series.iter().flatten().copied().fold(0.0f64, f64::max).max(1e-9);
    let tags: Vec<(String, bool)> = series.iter().map(|v| end_tag(v.last().copied().unwrap_or(0.0), unit(p), times.last())).collect();
    let w = area.width.saturating_sub(key_w + tags.iter().map(tag_width).max().unwrap_or(0) + 1);
    for (j, ((g, _), vals)) in groups.iter().zip(&series).enumerate() {
        let y = area.y + j as u16 * each;
        f.buffer_mut().set_string(area.x, y + each / 2, cut(g, key_w as usize - 2), Style::default().fg(SERIES[j]));
        let end = trace(f, Rect { x: area.x + key_w, y, width: w, height: each }, vals, 0.0, hi, SERIES[j]);
        put_tag(f, area.x + key_w + w + 1, end, &tags[j], SERIES[j]);
    }
    if dates && !times.is_empty() {
        let y = area.y + area.height - 1;
        let (a, z) = (short_date(&times[0]), short_date(&times[times.len() - 1]));
        f.buffer_mut().set_string(area.x + key_w, y, &a, Style::default().fg(OVERLAY0));
        let zw = z.chars().count() as u16;
        if w > zw + 8 {
            f.buffer_mut().set_string(area.x + key_w + w - zw, y, &z, Style::default().fg(OVERLAY0));
        }
    }
}

/// Values ranked: a thin rule a value, its end cut at half a cell, the
/// number after it; nothing filled.
fn bars(f: &mut Frame, area: Rect, p: &Panel) {
    // a line between rules where there is room for it, as rows on a screen breathe
    let gap = p.rows.len() * 2 <= area.height as usize + 1;
    let fit = if gap { (area.height as usize).div_ceil(2) } else { area.height as usize };
    let rows: Vec<(String, f64)> = p.rows.iter().take(fit).map(|r| (label(&r[0]), num(&r[r.len() - 1]).unwrap_or(0.0))).collect();
    let hi = rows.iter().map(|x| x.1).fold(0.0f64, f64::max).max(1e-9);
    let lw = rows.iter().map(|x| x.0.chars().count()).max().unwrap_or(0).min(16);
    let room = (area.width as usize).saturating_sub(lw + 9).max(1);
    let colour = role_colour(&p.role).max_blue();
    let lines: Vec<Line> = rows.iter().map(|(g, v)| {
        let halves = ((v.max(0.0) / hi) * (room * 2) as f64).round().max(if *v > 0.0 { 1.0 } else { 0.0 }) as usize;
        let rule = format!("{}{}", "━".repeat(halves / 2), if halves % 2 == 1 { "╸" } else { "" });
        Line::from(vec![
            Span::styled(format!("{:<lw$}  ", g.chars().take(lw).collect::<String>()), Style::default().fg(SUBTEXT)),
            Span::styled(rule, Style::default().fg(colour)),
            dim(format!(" {}", fmt(*v))),
        ])
    }).collect();
    let lines: Vec<Line> = if gap { lines.into_iter().flat_map(|l| [l, Line::raw("")]).collect() } else { lines };
    f.render_widget(Paragraph::new(lines), area);
}

fn add_list(f: &mut Frame, app: &mut App, body: Rect) {
    let w = (body.width * 7 / 10).max(40).min(body.width);
    let h = (body.height * 9 / 10).max(6).min(body.height);
    let rect = Rect { x: body.x + (body.width - w) / 2, y: body.y + (body.height - h) / 2, width: w, height: h };
    let items: Vec<ListItem> = app.offers().into_iter().map(|(r, p)| {
        ListItem::new(Line::from(vec![Span::styled("● ", Style::default().fg(role_colour(&r))), Span::styled(p.title, Style::default().fg(SUBTEXT))]))
    }).collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SURFACE1))
                .padding(Padding::new(2, 2, 1, 1))
                .title(Line::from(vec![Span::styled(" add a panel ", Style::default().fg(TEXT).add_modifier(Modifier::BOLD)), dim("enter add · esc back ")])),
        )
        .highlight_style(Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD))
        .highlight_symbol("› ");
    f.render_widget(Clear, rect);
    f.render_stateful_widget(list, rect, &mut app.pick);
}


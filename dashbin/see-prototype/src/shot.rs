//! A frame as a picture: the screen drawn into ratatui's test buffer and each
//! cell rasterised as a terminal would draw it — a monospace font for text,
//! and the characters a terminal draws itself, box lines, blocks and Braille
//! dots, drawn here as shapes so lines meet and dots are round.

use std::path::Path;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use image::{Rgb, RgbImage};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

// Windows Terminal's default scheme, Campbell, so a picture looks as the screen does there
const BG: [u8; 3] = [12, 12, 12];
const FG: [u8; 3] = [204, 204, 204];
const PAD: u32 = 28;
const MONO: [&str; 4] = [
    "C:/Windows/Fonts/CascadiaMono.ttf",
    "C:/Windows/Fonts/consola.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
];
const SYMBOLS: [&str; 3] = [
    "C:/Windows/Fonts/seguisym.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/System/Library/Fonts/Apple Symbols.ttf",
];

fn load(paths: &[&str]) -> Option<FontVec> {
    paths.iter().find_map(|p| std::fs::read(p).ok().and_then(|b| FontVec::try_from_vec(b).ok()))
}

fn rgb(c: Color, default: [u8; 3]) -> [u8; 3] {
    match c {
        Color::Reset => default,
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Black => [12, 12, 12],
        Color::White => [242, 242, 242],
        Color::Gray => [204, 204, 204],
        Color::DarkGray => [118, 118, 118],
        Color::Red => [197, 15, 31],
        Color::LightRed => [231, 72, 86],
        Color::Green => [19, 161, 14],
        Color::LightGreen => [22, 198, 12],
        Color::Yellow => [193, 156, 0],
        Color::LightYellow => [249, 241, 165],
        Color::Blue => [0, 55, 218],
        Color::LightBlue => [59, 120, 255],
        Color::Magenta => [136, 23, 152],
        Color::LightMagenta => [180, 0, 158],
        Color::Cyan => [58, 150, 221],
        Color::LightCyan => [97, 214, 214],
        Color::Indexed(_) => default,
    }
}

struct Canvas {
    img: RgbImage,
}

impl Canvas {
    fn blend(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x as u32 >= self.img.width() || y as u32 >= self.img.height() || a <= 0.0 {
            return;
        }
        let p = self.img.get_pixel_mut(x as u32, y as u32);
        let a = a.min(1.0);
        for k in 0..3 {
            p.0[k] = (p.0[k] as f32 * (1.0 - a) + c[k] as f32 * a).round() as u8;
        }
    }

    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 3]) {
        for y in y0.floor() as i64..y1.ceil() as i64 {
            for x in x0.floor() as i64..x1.ceil() as i64 {
                // coverage of the pixel by the rectangle, so edges are soft
                let cx = ((x as f32 + 1.0).min(x1) - (x as f32).max(x0)).clamp(0.0, 1.0);
                let cy = ((y as f32 + 1.0).min(y1) - (y as f32).max(y0)).clamp(0.0, 1.0);
                self.blend(x, y, c, cx * cy);
            }
        }
    }

    fn dot(&mut self, cx: f32, cy: f32, r: f32, c: [u8; 3]) {
        for y in (cy - r - 1.0).floor() as i64..=(cy + r + 1.0).ceil() as i64 {
            for x in (cx - r - 1.0).floor() as i64..=(cx + r + 1.0).ceil() as i64 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                self.blend(x, y, c, (r + 0.5 - d).clamp(0.0, 1.0));
            }
        }
    }
}

pub fn write(buf: &Buffer, path: &Path, px: f32) -> Result<(), String> {
    let mono = load(&MONO).ok_or("no monospace font found")?;
    let symbols = load(&SYMBOLS);
    let scale = PxScale::from(px);
    let sm = mono.as_scaled(scale);
    let cw = sm.h_advance(mono.glyph_id('M'));
    let ch = (px * 1.42).round();
    let base = ((ch - (sm.ascent() - sm.descent())) / 2.0 + sm.ascent()).round();
    let (w, h) = (buf.area.width as u32, buf.area.height as u32);
    let mut cv = Canvas {
        img: RgbImage::from_pixel((w as f32 * cw).ceil() as u32 + 2 * PAD, (h as f32 * ch) as u32 + 2 * PAD, Rgb(BG)),
    };
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let Some(cell) = buf.cell((x, y)) else { continue };
            let (mut fg, mut bg) = (rgb(cell.fg, FG), rgb(cell.bg, BG));
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            // faint, as a terminal draws it: the colour halfway to the background
            if cell.modifier.contains(Modifier::DIM) {
                fg = [0, 1, 2].map(|k| ((fg[k] as u16 + bg[k] as u16) / 2) as u8);
            }
            let (x0, y0) = (PAD as f32 + x as f32 * cw, PAD as f32 + y as f32 * ch);
            if bg != BG {
                cv.rect(x0, y0, x0 + cw, y0 + ch, bg);
            }
            let Some(c) = cell.symbol().chars().next() else { continue };
            if c == ' ' {
                continue;
            }
            if shape(&mut cv, c, x0, y0, cw, ch, fg) {
                continue;
            }
            let (font, id): (&FontVec, _) = match mono.glyph_id(c) {
                id if id.0 != 0 => (&mono, id),
                _ => match &symbols {
                    Some(s) if s.glyph_id(c).0 != 0 => (s, s.glyph_id(c)),
                    _ => (&mono, mono.glyph_id(c)),
                },
            };
            let glyph = id.with_scale_and_position(scale, point(x0, y0 + base));
            if let Some(o) = font.outline_glyph(glyph) {
                let b = o.px_bounds();
                let bold = cell.modifier.contains(Modifier::BOLD);
                o.draw(|gx, gy, cov| {
                    let (ix, iy) = (b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64);
                    // bold as a terminal fakes it, the glyph struck again a pixel over
                    cv.blend(ix, iy, fg, cov);
                    if bold {
                        cv.blend(ix + 1, iy, fg, cov * 0.65);
                    }
                });
            }
        }
    }
    cv.img.save(path).map_err(|e| e.to_string())
}

/// The characters a terminal draws as shapes: box lines, blocks, Braille.
fn shape(cv: &mut Canvas, c: char, x0: f32, y0: f32, cw: f32, ch: f32, fg: [u8; 3]) -> bool {
    let u = c as u32;
    if (0x2800..=0x28FF).contains(&u) {
        // Braille: two columns of four dots, bits 1 2 3 7 down the left, 4 5 6 8 down the right
        let bits = u - 0x2800;
        const AT: [(u32, f32, f32); 8] = [(0, 0.0, 0.0), (1, 0.0, 1.0), (2, 0.0, 2.0), (3, 1.0, 0.0), (4, 1.0, 1.0), (5, 1.0, 2.0), (6, 0.0, 3.0), (7, 1.0, 3.0)];
        let r = (cw * 0.2).max(1.3);
        for (bit, col, row) in AT {
            if bits & (1 << bit) != 0 {
                cv.dot(x0 + cw * (0.28 + 0.44 * col), y0 + ch * (0.14 + 0.24 * row), r, fg);
            }
        }
        return true;
    }
    let (mx, my, t) = (x0 + cw / 2.0, y0 + ch / 2.0, (cw / 7.0).max(1.0));
    let heavy = 2.0 * t;
    match c {
        // rounded corners as a quarter of an ellipse round the cell's corner between the two ends it joins
        '╭' | '╮' | '╰' | '╯' => {
            let (cx, cy) = match c { '╭' => (x0 + cw, y0 + ch), '╮' => (x0, y0 + ch), '╰' => (x0 + cw, y0), _ => (x0, y0) };
            let (rx, ry) = (cw / 2.0, ch / 2.0);
            for k in 0..240 {
                let a = k as f32 / 240.0 * std::f32::consts::TAU;
                let (px, py) = (cx + rx * a.cos(), cy + ry * a.sin());
                if px >= x0 - 0.01 && px <= x0 + cw + 0.01 && py >= y0 - 0.01 && py <= y0 + ch + 0.01 {
                    cv.rect(px - t / 2.0, py - t / 2.0, px + t / 2.0, py + t / 2.0, fg);
                }
            }
        }
        '╸' => cv.rect(x0, my - heavy / 2.0, mx, my + heavy / 2.0, fg),
        // blocks as a terminal draws them: a full cell, a left or a lower part of it, touching its neighbours
        '█' => cv.rect(x0 - 0.5, y0 - 0.5, x0 + cw + 0.5, y0 + ch + 0.5, fg),
        '▏' | '▎' | '▍' | '▌' | '▋' | '▊' | '▉' => {
            let n = match c { '▏' => 1.0, '▎' => 2.0, '▍' => 3.0, '▌' => 4.0, '▋' => 5.0, '▊' => 6.0, _ => 7.0 };
            cv.rect(x0 - 0.5, y0 - 0.5, x0 + cw * n / 8.0, y0 + ch + 0.5, fg);
        }
        '▁' | '▂' | '▃' | '▄' | '▅' | '▆' | '▇' => {
            let n = match c { '▁' => 1.0, '▂' => 2.0, '▃' => 3.0, '▄' => 4.0, '▅' => 5.0, '▆' => 6.0, _ => 7.0 };
            cv.rect(x0 - 0.5, y0 + ch * (1.0 - n / 8.0), x0 + cw + 0.5, y0 + ch + 0.5, fg);
        }
        '━' => cv.rect(x0, my - heavy / 2.0, x0 + cw, my + heavy / 2.0, fg),
        '─' => cv.rect(x0, my - t / 2.0, x0 + cw, my + t / 2.0, fg),
        '│' => cv.rect(mx - t / 2.0, y0, mx + t / 2.0, y0 + ch, fg),
        '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' => {
            let (l, r, up, dn) = match c {
                '┌' => (false, true, false, true),
                '┐' => (true, false, false, true),
                '└' => (false, true, true, false),
                '┘' => (true, false, true, false),
                '├' => (false, true, true, true),
                '┤' => (true, false, true, true),
                '┬' => (true, true, false, true),
                '┴' => (true, true, true, false),
                _ => (true, true, true, true),
            };
            if l { cv.rect(x0, my - t / 2.0, mx + t / 2.0, my + t / 2.0, fg) }
            if r { cv.rect(mx - t / 2.0, my - t / 2.0, x0 + cw, my + t / 2.0, fg) }
            if up { cv.rect(mx - t / 2.0, y0, mx + t / 2.0, my + t / 2.0, fg) }
            if dn { cv.rect(mx - t / 2.0, my - t / 2.0, mx + t / 2.0, y0 + ch, fg) }
        }
        _ => return false,
    }
    true
}

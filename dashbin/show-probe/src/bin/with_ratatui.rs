//! The same dashboard drawn by ratatui's widgets into a buffer, printed in the terminal's sixteen colours, its width
//! the terminal's: a number, a Braille line, bars and a table.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Constraint, Rect};
use ratatui_core::style::{Color, Style};
use ratatui_core::symbols::Marker;
use ratatui_core::widgets::Widget;
use ratatui_widgets::barchart::BarChart;
use ratatui_widgets::block::Block;
use ratatui_widgets::chart::{Axis, Chart, Dataset, GraphType};
use ratatui_widgets::paragraph::Paragraph;
use ratatui_widgets::table::{Row, Table};

fn main() {
    let width = terminal_size::terminal_size().map_or(80, |(w, _)| w.0).max(40);
    let area = Rect::new(0, 0, width, 38);
    let mut buffer = Buffer::empty(area);
    Paragraph::new("312").block(Block::bordered().title("Orders today")).render(Rect::new(0, 0, 20, 4), &mut buffer);
    let points: Vec<(f64, f64)> = (0..30).map(|i| (f64::from(i), (f64::from(i) / 4.0).sin() * 100.0 + 150.0)).collect();
    let line = Dataset::default()
        .name("revenue")
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::new().fg(Color::LightBlue))
        .data(&points);
    Chart::new(vec![line])
        .block(Block::bordered().title("Revenue, 30 days"))
        .x_axis(Axis::default().bounds([0.0, 29.0]))
        .y_axis(Axis::default().bounds([0.0, 300.0]).labels(["0", "150", "300"]))
        .render(Rect::new(20, 0, width - 20, 12), &mut buffer);
    BarChart::default()
        .data(&[("paid", 120), ("refunded", 7), ("open", 30)])
        .bar_width(8)
        .bar_style(Style::new().fg(Color::LightGreen))
        .block(Block::bordered().title("Status"))
        .render(Rect::new(0, 12, width / 2, 12), &mut buffer);
    let rows = (0..10).map(|i| Row::new(vec![i.to_string(), format!("{:.2}", f64::from(i) * 100.5), "paid".to_owned()]));
    Table::new(rows, [Constraint::Length(6), Constraint::Length(10), Constraint::Fill(1)])
        .header(Row::new(vec!["id", "total", "status"]))
        .block(Block::bordered().title("Last orders"))
        .render(Rect::new(0, 24, width, 14), &mut buffer);
    print(&buffer);
}

fn print(buffer: &Buffer) {
    let area = buffer.area;
    for y in 0..area.height {
        let mut line = String::new();
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            let code = match cell.fg {
                Color::LightBlue => "\x1b[94m",
                Color::LightGreen => "\x1b[92m",
                _ => "\x1b[0m",
            };
            line.push_str(code);
            line.push_str(cell.symbol());
        }
        println!("{line}\x1b[0m");
    }
}

//! A dashboard printed with format! alone: what the command line already links, to weigh ratatui against.

fn main() {
    let width: usize = std::env::args().nth(1).and_then(|w| w.parse().ok()).unwrap_or(80);
    let line = "─".repeat(width.saturating_sub(2));
    println!("┌{line}┐");
    println!("│ {:<w$} │", "Orders today  312", w = width.saturating_sub(4));
    println!("└{line}┘");
    for (i, total) in [120.5_f64, 7.25, 30.0].iter().enumerate() {
        println!("{i:>6} {total:>10.2} {:<10}", "paid");
    }
    let points: Vec<f64> = (0..30).map(|i| (f64::from(i) / 4.0).sin() * 100.0 + 150.0).collect();
    let dots: String = points.iter().map(|v| if *v > 150.0 { '⠉' } else { '⣀' }).collect();
    println!("{dots}");
}

//! The release round's archive cases, run by run.py as its Bun programs are:
//!
//!   archive metrics --corpus wide.lp --contender tinystore|victoria|prometheus --dir <d>
//!   archive prepare --corpus /corpus/loghub --dataset HDFS_v1 --out /corpus/prepared
//!   archive logs --prepared /corpus/prepared --dataset HDFS_v1 --contender tinystore|loki --dir <d>
//!   archive commits --dir <d> --count 2000

mod commits;
mod logs;
mod metrics;
mod out;

fn main() {
    let outcome = out::Args::parse().and_then(|(command, args)| match command.as_str() {
        "metrics" => metrics::run(&args),
        "commits" => commits::run(&args),
        "prepare" => logs::prepare(&args),
        "logs" => logs::run(&args),
        other => Err(format!("no command {other}").into()),
    });
    if let Err(error) = outcome {
        eprintln!("archive: {error}");
        std::process::exit(1);
    }
}

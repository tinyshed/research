mod alternatives;
mod common;
mod fixture;
#[cfg(feature = "product")]
mod integration;
mod native;
mod quality;
mod rebuild;
#[cfg(feature = "text")]
mod text;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        "native" => native::run(&args[1..]),
        "rebuild" => rebuild::run(&args[1..]),
        "rebuild-check" => rebuild::check(&args[1..]),
        "crash" => rebuild::crash(&args[1..]),
        "recover" => rebuild::recover(&args[1..]),
        "quality" => quality::run(),
        "alternative" => alternatives::run(&args[1..]),
        "prepare" => {
            let c = common::open(&common::path());
            fixture::seed(&c, args[2].parse().unwrap(), &args[3], &args[4], 1000);
            if args[1] != "source" {
                native::initialize_active(&c, &args[1], true);
            }
            println!(
                "{}",
                serde_json::json!({"experiment":"prepare","rows":args[2],"kind":args[1],"status":"pass"})
            );
        }
        #[cfg(feature = "text")]
        "tantivy" => text::run(&args[1..]),
        #[cfg(feature = "product")]
        "locks" => integration::locks(&args[1..]),
        #[cfg(feature = "product")]
        "lease-check" => integration::check(),
        #[cfg(feature = "product")]
        "lease-crash" => integration::crash(),
        #[cfg(feature = "product")]
        "lease-recover" => integration::recover(),
        _ => panic!("unknown experiment"),
    }
}

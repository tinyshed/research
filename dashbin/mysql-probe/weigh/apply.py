import pathlib, shutil, sys

tree = pathlib.Path(sys.argv[1])
variant = sys.argv[2]
here = pathlib.Path(__file__).parent
deps = {
    "mysql_async": 'mysql_async = { version = "0.37.1", default-features = false, features = ["minimal-rust"] }\n',
    "mysql_async_tls": 'mysql_async = { version = "0.37.1", default-features = false, features = ["minimal-rust", "rustls-tls", "ring", "tls12"] }\n',
    "sqlx": 'sqlx-mysql = { version = "0.9.0", default-features = false, features = ["rsa"] }\n'
            'sqlx-core = { version = "0.9.0", default-features = false, features = ["_rt-tokio", "_tls-none"] }\n',
}[variant]
cargo = tree / "crates/cli/Cargo.toml"
text = cargo.read_text(encoding="utf-8")
text = text.replace("[lints]", deps + "\n[lints]", 1)
if variant == "mysql_async_tls":
    text += '\n[features]\ntls = []\ndefault = ["tls"]\n'
cargo.write_text(text, encoding="utf-8", newline="\n")
source = "sqlx.rs" if variant == "sqlx" else "mysql_async.rs"
shutil.copy(here / source, tree / "crates/cli/src/weight.rs")
main = tree / "crates/cli/src/main.rs"
text = main.read_text(encoding="utf-8")
text = text.replace("mod table;\n", "mod table;\nmod weight;\n", 1)
call = """fn main() -> ExitCode {
    if let Ok(target) = std::env::var("DASHBIN_WEIGHT") {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
        let read = runtime.block_on(weight::mysql(&target));
        return if read.map(|seen| seen > 0).unwrap_or(false) { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
"""
assert text.count("fn main() -> ExitCode {\n") == 1
text = text.replace("fn main() -> ExitCode {\n", call, 1)
main.write_text(text, encoding="utf-8", newline="\n")
print("applied", variant)

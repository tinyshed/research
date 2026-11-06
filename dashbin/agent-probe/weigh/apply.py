"""Turn a detached worktree of dashbin into one variant of the weighing: its dependencies in the binary's
Cargo.toml, the harness as crates/cli/src/weight.rs, and a hook in main that runs it when DASHBIN_WEIGHT is set,
so that nothing it links is dropped as unused.

    python apply.py <tree> <variant>
"""

import pathlib
import sys

tree = pathlib.Path(sys.argv[1])
variant = sys.argv[2]
here = pathlib.Path(__file__).parent

AXUM = 'axum = { version = "0.8.9", default-features = false, features = ["ws", "tokio", "http1"] }\n'
SINK = 'futures-util = { version = "0.3", default-features = false, features = ["sink"] }\n'
PLAIN = 'tokio-tungstenite = { version = "0.29.0", default-features = false, features = ["connect"] }\n'
RUSTLS_WS = 'tokio-tungstenite = { version = "0.29.0", default-features = false, features = ["connect", "__rustls-tls"] }\n'
RING = 'rustls = { version = "0.23.35", default-features = false, features = ["ring", "std", "tls12"] }\n'
AWS_LC = 'rustls = { version = "0.23.35", default-features = false, features = ["aws_lc_rs", "std", "tls12"] }\n'
WEBPKI = 'webpki-roots = "1.0.4"\n'
NATIVE = 'rustls-native-certs = "0.8.2"\n'

RING_PROVIDER = "\nfn provider() -> rustls::crypto::CryptoProvider {\n    rustls::crypto::ring::default_provider()\n}\n"
AWS_PROVIDER = "\nfn provider() -> rustls::crypto::CryptoProvider {\n    rustls::crypto::aws_lc_rs::default_provider()\n}\n"
WEBPKI_ROOTS = (
    "\nfn roots() -> rustls::RootCertStore {\n"
    "    rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() }\n}\n"
)
NATIVE_ROOTS = (
    "\nfn roots() -> rustls::RootCertStore {\n"
    "    let mut roots = rustls::RootCertStore::empty();\n"
    "    roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);\n"
    "    roots\n}\n"
)

# variant: (dependencies, harness parts, what run() calls)
VARIANTS = {
    "hub_ws": (AXUM, ["hub.rs"], ["hub"]),
    "agent_plain": (SINK + PLAIN, ["agent.rs", "tls_none.rs"], ["agent"]),
    "agent_ring_webpki": (SINK + RUSTLS_WS + RING + WEBPKI, ["agent.rs", "tls_rustls.rs", RING_PROVIDER, WEBPKI_ROOTS], ["agent"]),
    "agent_ring_native": (SINK + RUSTLS_WS + RING + NATIVE, ["agent.rs", "tls_rustls.rs", RING_PROVIDER, NATIVE_ROOTS], ["agent"]),
    "agent_awslc_webpki": (SINK + RUSTLS_WS + AWS_LC + WEBPKI, ["agent.rs", "tls_rustls.rs", AWS_PROVIDER, WEBPKI_ROOTS], ["agent"]),
    "hub_and_agent": (
        AXUM + SINK + RUSTLS_WS + RING + NATIVE,
        ["hub.rs", "agent.rs", "tls_rustls.rs", RING_PROVIDER, NATIVE_ROOTS],
        ["hub", "agent"],
    ),
}

dependencies, parts, calls = VARIANTS[variant]

cargo = tree / "crates/cli/Cargo.toml"
text = cargo.read_text(encoding="utf-8")
assert text.count("[lints]") == 1
cargo.write_text(text.replace("[lints]", dependencies + "\n[lints]", 1), encoding="utf-8", newline="\n")

harness = []
for part in parts:
    source = (here / part).read_text(encoding="utf-8") if part.endswith(".rs") else part
    # parts are joined into one module, where only the first may open with its own doc
    harness.append(source if not harness else source.replace("//!", "//"))
# the hub serves until it is stopped, so a variant with both dials its own hub
body = {
    ("hub",): "    hub(target).await",
    ("agent",): "    agent(target).await",
    ("hub", "agent"): (
        '    let served = tokio::spawn(async { hub("127.0.0.1:47317").await });\n'
        "    let dialled = agent(target).await;\n"
        "    served.abort();\n"
        "    dialled"
    ),
}[tuple(calls)]
harness.append(f"\npub async fn run(target: &str) -> bool {{\n{body}\n}}\n")
(tree / "crates/cli/src/weight.rs").write_text("".join(harness), encoding="utf-8", newline="\n")

main = tree / "crates/cli/src/main.rs"
text = main.read_text(encoding="utf-8")
hook = """fn main() -> ExitCode {
    if let Ok(target) = std::env::var("DASHBIN_WEIGHT") {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
        return if runtime.block_on(weight::run(&target)) { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
"""
assert text.count("fn main() -> ExitCode {\n") == 1 and text.count("mod table;\n") == 1
text = text.replace("mod table;\n", "mod table;\nmod weight;\n", 1).replace("fn main() -> ExitCode {\n", hook, 1)
main.write_text(text, encoding="utf-8", newline="\n")
print("applied", variant)

//! Builds vec1.c, which fetch-vec1.sh fetches, into the lab when the `vec1`
//! feature asks for it: statically, against the SQLite rusqlite bundles, with
//! the AVX2 kernels its documentation builds it with.

fn main() {
    println!("cargo:rerun-if-env-changed=VEC1_C");
    if std::env::var_os("CARGO_FEATURE_VEC1").is_none() {
        return;
    }
    let source = std::env::var("VEC1_C")
        .expect("the vec1 feature needs VEC1_C, the path fetch-vec1.sh wrote");
    println!("cargo:rerun-if-changed={source}");
    cc::Build::new()
        .file(&source)
        .include(
            std::env::var("DEP_SQLITE3_INCLUDE")
                .expect("libsqlite3-sys names its header's directory"),
        )
        .define("VEC1_STATIC", None)
        .define("NDEBUG", None)
        .opt_level(3)
        .flag("-mavx2")
        .flag("-mfma")
        .warnings(false)
        .compile("vec1");
}

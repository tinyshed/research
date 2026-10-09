fn main() {
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=pthread");
}

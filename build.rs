fn main() {
    if std::env::var_os("CARGO_FEATURE_NUMA").is_some() {
        println!("cargo:rustc-link-lib=numa");
    }
}

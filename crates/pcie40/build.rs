fn main() {
    build_pcie40_bindings();
}

fn build_pcie40_bindings() {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    const PCIE40_WRAPPER_H: &str = "src/wrapper.h";
    const PCIE40_BINDINGS: &str = "src/bindings.rs";
    const PCIE40_LIBS: &[&str] = &["pcie40_daq", "pcie40_id"];
    const NO_BINDGEN: &str = "NO_BINDGEN";
    

    println!("cargo:rerun-if-changed={PCIE40_WRAPPER_H}");

    // Link to the p40 libraries
    for lib in PCIE40_LIBS {
        println!("cargo:rustc-link-lib={lib}");
    }

    // skip rest if running bindgen is disabled
    println!("cargo::rerun-if-env-changed={NO_BINDGEN}");
    let run_bind = env::var_os(NO_BINDGEN).is_none_or(|v| v.eq_ignore_ascii_case("false"));
    if !run_bind {
        println!(
            "cargo::warning=Running bindgen and linking to the pcie40 c libraries is **disabled**, as {NO_BINDGEN} is set (to true).\nThis is only useful for `cargo check`."
        );
        return;
    }

    // Tell cargo to re-run if the wrapper.h changes

    // Generate bindings using the wrapper header
    let bindings = bindgen::Builder::default()
        .header(PCIE40_WRAPPER_H)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate pcie40 bindings");

    // Create the output directory structure if it doesn't exist
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let output_path = manifest_dir.join(PCIE40_BINDINGS);

    // Ensure the parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create pcie40 bindings output directory");
    }

    // Write the bindings to the file
    bindings
        .write_to_file(&output_path)
        .unwrap_or_else(|_| panic!("Couldn't write pcie40 bindings to {PCIE40_BINDINGS}!"));

    println!("cargo:warning=Generated pcie40 bindings at {PCIE40_BINDINGS}");
}

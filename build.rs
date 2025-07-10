fn main() {
    #[cfg(feature = "pcie40")]
    build_pcie40_bindings();
}

#[cfg(feature = "pcie40")]
fn build_pcie40_bindings() {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    const PCIE40_WRAPPER_H: &str = "src/pcie40/wrapper.h";
    const PCIE40_BINDINGS: &str = "src/pcie40/bindings.rs";
    const PCIE40_LIBS: &[&str] = &["pcie40_daq", "pcie40_id"];


    // Tell cargo to re-run if the wrapper.h changes
    println!("cargo:rerun-if-changed={PCIE40_WRAPPER_H}");

    // Link to the p40 libraries
    for lib in PCIE40_LIBS {
        println!("cargo:rustc-link-lib={lib}");
    }

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

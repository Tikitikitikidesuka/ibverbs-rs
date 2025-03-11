use std::env;
use std::path::PathBuf;
use std::fs;

fn main() {
    // Tell cargo to re-run if the wrapper.h changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // Link to the p40 libraries
    println!("cargo:rustc-link-lib=pcie40_daq");
    println!("cargo:rustc-link-lib=pcie40_id");

    // Generate bindings using the wrapper header
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Write to src directory for IDE visibility
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src_dir = manifest_dir.join("src");

    // Make sure src directory exists
    fs::create_dir_all(&src_dir).expect("Failed to create src directory");

    bindings
        .write_to_file(src_dir.join("bindings.rs"))
        .expect("Couldn't write bindings to src directory!");

    println!("cargo:warning=Wrote bindings to src/bindings.rs");
}
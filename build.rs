use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to re-run if the wrapper.h changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // Link to the p40 libraries
    println!("cargo:rustc-link-lib=pcie40_daq");

    // Generate bindings using the wrapper header
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

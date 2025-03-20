use env_logger::Builder;
use log::{debug, LevelFilter};
//use pcie40_rs::mfp_reader::PCIe40MFPReader;

fn main() {
    // Initialize logger with debug level
    Builder::new()
        .filter_level(LevelFilter::Debug)
        .init();

    debug!("Logger initialized with debug level");

    //let reader = PCIe40MFPReader::open_by_device_name("tdtel203_0", 1).unwrap();
}
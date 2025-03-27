//use env_logger::Builder;
//use log::{debug, LevelFilter};
//use pcie40_rs::mfp_reader::PCIe40MFPReader;

use pcie40_rs::pcie40_id::PCIe40Id;

fn main() {
    //let reader = ZeroCopyReader::new(PCIe40ZeroCopyReaderImpl::new());
    let mut dev_id = PCIe40Id::open_by_device_name("tdtel201_0").unwrap();
    println!("{}", dev_id.unique_device_name().unwrap());
    println!("{}", dev_id.fpga_serial_number().unwrap());
}

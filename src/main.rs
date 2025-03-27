//use env_logger::Builder;
//use log::{debug, LevelFilter};
//use pcie40_rs::mfp_reader::PCIe40MFPReader;

use pcie40_rs::pcie40::PCIe40Id;
use pcie40_rs::pcie40_reader::PCIe40ZeroCopyReaderImpl;

fn main() {
    //let reader = ZeroCopyReader::new(PCIe40ZeroCopyReaderImpl::new());
    PCIe40Id::open_by_device_name("tdeb20")
}

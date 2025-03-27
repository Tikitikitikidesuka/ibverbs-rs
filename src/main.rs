//use env_logger::Builder;
//use log::{debug, LevelFilter};
//use pcie40_rs::mfp_reader::PCIe40MFPReader;

use pcie40_rs::pcie40_ctrl::PCIe40Ctrl;
use pcie40_rs::pcie40_id::PCIe40Id;
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40Stream;

fn main() {
    //let reader = ZeroCopyReader::new(PCIe40ZeroCopyReaderImpl::new());
    let mut id_endpt = PCIe40Id::open_by_device_name("tdtel201_0").unwrap();
    println!("{}", id_endpt.unique_device_name().unwrap());
    println!("{}", id_endpt.fpga_serial_number().unwrap());

    let mut ctrl_endpt = PCIe40Ctrl::open_by_device_name("tdtel201_0").unwrap();

    let mut stream_endpt = PCIe40Stream::open_by_device_name("tdtel201_0", MainStream).unwrap();
}

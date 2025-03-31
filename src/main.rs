//use env_logger::Builder;
//use log::{debug, LevelFilter};
//use pcie40_rs::mfp_reader::PCIe40MFPReader;

use std::thread;
use std::time::Duration;
use pcie40_rs::pcie40_ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40_id::PCIe40IdManager;
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::{MetaFormat, RawFormat};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;

fn main() {
    //let reader = ZeroCopyReader::new(PCIe40ZeroCopyReaderImpl::new());
    /*
    let mut id_endpt = PCIe40Id::open_by_device_name("tdtel201_0").unwrap();
    println!("{}", id_endpt.unique_device_name().unwrap());
    println!("{}", id_endpt.fpga_serial_number().unwrap());

    let mut ctrl_endpt = PCIe40Ctrl::open_by_device_name("tdtel203_0").unwrap();
    */

    let mut stream = PCIe40StreamManager::open_by_device_name("tdtel203_0", MainStream, MetaFormat).unwrap();
    let mut stream_guard = stream.lock();
}

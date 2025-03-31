//use env_logger::Builder;
//use log::{debug, LevelFilter};
//use pcie40_rs::mfp_reader::PCIe40MFPReader;

use pcie40_rs::pcie40_ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40_id::PCIe40IdManager;
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::{MetaFormat, RawFormat};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;
use std::thread;
use std::time::Duration;

fn main() {
    let mut stream =
        PCIe40StreamManager::open_by_device_name("tdtel203_0", MainStream, MetaFormat).unwrap();
    let mut stream_guard = stream.lock().unwrap();
    let buffer = stream_guard.map_buffer().unwrap();
}

use std::io::{stdin, Read};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::{MetaFormat, RawFormat};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;
use log::{info, debug, error};
use env_logger::{Env, Builder};

fn main() {
    Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let mut stream =
        PCIe40StreamManager::open_by_device_name("tdtel203_0", MainStream, MetaFormat).unwrap();
    let mut stream_guard = stream.lock().unwrap();
    println!("Stream configured... Press any key to proceed");
    stdin().read(&mut [0]).unwrap();
    let buffer = stream_guard.map_buffer().unwrap();
    println!("Buffer: {:x?}", &buffer.data()[..1024]);
}

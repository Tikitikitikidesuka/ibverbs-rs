use std::io::{stdin, Read};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::{MetaFormat, RawFormat};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;
use log::{info, debug, error};
use env_logger::{Env, Builder};
use pcie40_rs::bindings::p40_stream_get_host_buf_read_off;
use pcie40_rs::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;

fn main() {
    Builder::from_env(Env::default().default_filter_or("trace"))
        .format_timestamp_secs()
        .init();

    let mut stream =
        PCIe40StreamManager::open_by_device_name("tdtel203_0", MainStream, MetaFormat).unwrap();
    stream.set_raii_enable_state_close_mode(PreserveEnableState).unwrap();
    let mut stream_guard = stream.lock().unwrap();
    println!("Stream configured... Press any key to proceed");
    stdin().read(&mut [0]).unwrap();
    let buffer = stream_guard.map_buffer().unwrap();
    let read_offset = buffer.get_read_offset().unwrap();
    let write_offset = buffer.get_write_offset().unwrap();
    println!("Buffer: {:x?}", unsafe { &buffer.data()[read_offset..write_offset][..1024] });
}

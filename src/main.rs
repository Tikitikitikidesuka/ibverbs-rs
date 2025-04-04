use std::io::{stdin, Read};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::{MetaFormat, RawFormat};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;
use env_logger::{Env, Builder};
use pcie40_rs::pcie40_reader::PCIe40Reader;
use pcie40_rs::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::test_readable::I32List;
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
use pcie40_rs::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

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
    let mut reader = PCIe40Reader::new(buffer).unwrap();

    /*
    println!("Reader data: {:x?}", &*reader.data());
    println!("Loaded {} bytes", reader.load_data(64).unwrap());
    println!("Reader data: {:x?}", &*reader.data());
    println!("Discarded {} bytes", reader.discard_data(32).unwrap());
    println!("Reader data: {:x?}", &*reader.data());
    */

    let i32_list_guard = I32List::load(&mut reader).unwrap();
    let i32_list = I32List::cast(&i32_list_guard).unwrap();
    println!("Read TestReadable: {:?}", i32_list);
}

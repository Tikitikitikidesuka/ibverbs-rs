use env_logger::{Builder, Env};
use pcie40_rs::demo_reader::DemoZeroCopyRingBufferReader;
use pcie40_rs::pcie40_reader::PCIe40Reader;
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::{MetaFormat, RawFormat};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;
use pcie40_rs::test_readable::I32ListRef;
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
use std::io::{Read, stdin};
use std::ops::Deref;
use pcie40_rs::pcie40_ctrl::PCIe40ControllerManager;
//use pcie40_rs::test_readable::I32List;
//use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
use pcie40_rs::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

fn main() {
    Builder::from_env(Env::default().default_filter_or("trace"))
        .format_timestamp_secs()
        .format_line_number(true)
        .init();

    let mut controller = PCIe40ControllerManager::open_by_device_name("tdtel203_0").unwrap();
    println!("Meta alignment: {}", controller.meta_alignment().unwrap());

    let mut stream =
        PCIe40StreamManager::open_by_device_name("tdtel203_0", MainStream, MetaFormat).unwrap();
    stream
        .set_raii_enable_state_close_mode(PreserveEnableState)
        .unwrap();

    let mut stream_guard = stream.lock().unwrap();
    let mut reader = PCIe40Reader::new(stream_guard.map_buffer().unwrap()).unwrap();
    println!("Discarding all data on the stream...");
    reader.discard_all_data().unwrap();

    println!("Stream configured and flushed... Press any key to proceed");
    stdin().read(&mut [0]).unwrap();

    /*
    let buffer = stream_guard.map_buffer().unwrap();
    let mut reader = PCIe40Reader::new(buffer).unwrap();
    */

    /*
    let demo_data: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

    let mut reader = DemoZeroCopyRingBufferReader::new(demo_data);

    println!("Reader data: {:?}", &*reader.data());
    println!("Loaded {} bytes", reader.load_data(4).unwrap());
    println!("Reader data: {:?}", &*reader.data());
    //println!("Discarded {} bytes", reader.discard_data(32).unwrap());
    println!("Discarding loaded data...");
    reader.data().discard().unwrap();
    println!("Reader data: {:?}", &*reader.data());
    println!("Loaded {} bytes", reader.load_data(4).unwrap());
    println!("Reader data: {:?}", &*reader.data());
    */

    /*
    let demo_data: Vec<u8> = [
        vec![0, 4, 0, 1, 2, 3],
        vec![1, 5, 4, 5, 6, 7, 8],
        vec![2, 3, 9, 10, 11],
        vec![3, 9, 12, 13, 14, 15, 16, 17, 18, 19, 20],
    ]
    .concat()
    .iter()
    .flat_map(|value: &i32| value.to_le_bytes())
    .collect();

    let mut reader = DemoZeroCopyRingBufferReader::new(demo_data);
    */

    /*
    let i32_list_guard = I32ListRef::load(&mut reader).unwrap();
    let i32_list = I32ListRef::cast(&*i32_list_guard).unwrap();
    */

    /*
    let i32_list = I32ListRef::read(&mut reader).unwrap();
    println!("Read TestReadable: {:?}", i32_list.deref());
    let i32_list = I32ListRef::read(&mut reader).unwrap();
    println!("Read TestReadable: {:?}", i32_list.deref());
    println!("Discarding...");
    i32_list.discard().unwrap();
    let i32_list = I32ListRef::read(&mut reader).unwrap();
    println!("Read TestReadable: {:?}", i32_list.deref());
    */


    /*
    println!("Loading 2 I32Lists...");
    let i32_list = I32ListRef::read_multiple(&mut reader, 2).unwrap();
    println!("Read TestReadable 0: {}", i32_list);
    println!("Read TestReadable 1: {}", i32_list);
    println!("Read TestReadables: {}", i32_list.iter());
    println!("Discarding...");
    i32_list.discard().unwrap();
    println!("Loading 2 I32Lists...");
    let i32_list = I32ListRef::read_multiple(&mut reader, 2).unwrap();
    println!("Read TestReadable 0: {}", i32_list[0]);
    println!("Read TestReadable 1: {}", i32_list[1]);
    */

    reader.load_data(1024).unwrap();
    let data_guard = reader.data();
    println!("Loaded data: {:x?}", data_guard);
}

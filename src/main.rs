use env_logger::{Builder, Env};
use pcie40_rs::multi_fragment_packet::MultiFragmentPacketRef;
use pcie40_rs::pcie40_ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40_reader::PCIe40Reader;
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::{MetaFormat, RawFormat};
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
use pcie40_rs::utils;
use pcie40_rs::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;
use std::io::{Read, stdin};

fn main() {
    const DEVICE_NAME: &str = "tdtel202_0";

    Builder::from_env(Env::default().default_filter_or("trace"))
        .format_timestamp_secs()
        .format_file(true)
        .format_line_number(true)
        .init();

    let mut controller = PCIe40ControllerManager::open_by_device_name(DEVICE_NAME).unwrap();
    let meta_alignment = controller.meta_alignment().unwrap();

    let mut stream =
        PCIe40StreamManager::open_by_device_name(DEVICE_NAME, MainStream, MetaFormat).unwrap();
    stream
        .set_raii_enable_state_close_mode(PreserveEnableState)
        .unwrap();

    let mut stream_guard = stream.lock().unwrap();
    let mut reader =
        PCIe40Reader::new(stream_guard.map_buffer().unwrap(), meta_alignment).unwrap();
    //println!("\n\nDiscarding all data on the stream...\n\n");
    //reader.discard_all_data().unwrap();

    println!("\n\nStream configured... Press any key to proceed\n");
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

    /*
    reader.load_data(32).unwrap();
    let data_guard = reader.data();
    println!("Loaded data: {:x?}", data_guard);
    */

    /*
    println!("Loading an MFP...");
    let mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    println!("Read MFP: {:?}", mfp.data_ref());
    println!("Discarding the MFP...");
    mfp.discard().unwrap();
    println!("Loading an MFP...");
    let mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    println!("Read MFP: {:?}", mfp.data_ref());
    */

    println!("Loading 2 MFPs...");
    let mfps = MultiFragmentPacketRef::read_multiple(&mut reader, 2).unwrap();
    println!("Read MFPs: {:?}", mfps);
    println!("Read MFP[0]: {:?}", mfps[0]);
    println!("Read MFP[1]: {:?}", mfps[1]);
    println!("Discarding MFPs...");
    mfps.discard().unwrap();
    println!("Loading 2 MFPs...");
    let mfps = MultiFragmentPacketRef::read_multiple(&mut reader, 2).unwrap();
    println!("Read MFPs: {:?}", mfps);
    println!("Read MFP[0]: {:?}", mfps[0]);
    println!("Read MFP[1]: {:?}", mfps[1]);
}

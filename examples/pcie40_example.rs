use std::io::{stdin, Read};
use pcie40_rs::pcie40::ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40::reader::PCIe40Reader;
use pcie40_rs::pcie40::stream::stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40_rs::pcie40::stream::stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40::stream::stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40::stream::stream::PCIe40StreamManager;
use pcie40_rs::utils;
use pcie40_rs::utils::IsPow2Result;

fn main() {
    const DEVICE_NAME: &str = "tdtel203_1";

    let controller = PCIe40ControllerManager::open_by_device_name(DEVICE_NAME).unwrap();
    let meta_alignment_pow2 = match utils::is_pow2(controller.meta_alignment().unwrap()) {
        IsPow2Result::Yes(pow2) => pow2,
        IsPow2Result::No => {
            panic!("Meta alignment is not a power of 2")
        }
    };

    let mut stream =
        PCIe40StreamManager::open_by_device_name(DEVICE_NAME, MainStream, MetaFormat).unwrap();
    stream
        .set_raii_enable_state_close_mode(PreserveEnableState)
        .unwrap();

    let mapped_stream = stream.lock().unwrap().map_buffer().unwrap();

    let mut reader = PCIe40Reader::new(mapped_stream, meta_alignment_pow2).unwrap();

    println!("\n\nStream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    // Create reader and writer
    let mut reader = DemoContiguousBufferReader::new(&mut demo_buffer);
    let mut writer = DemoContiguousBufferWriter::new(&mut demo_buffer);

    write_to_contiguous_buffer(&mut writer, b"0123456789ABCD").unwrap();
    print_contiguous_buffer(&reader);

    reader.advance_read_pointer(2).unwrap();
    print_contiguous_buffer(&reader);

    write_to_contiguous_buffer(&mut writer, b"EFGH").unwrap();
    print_contiguous_buffer(&reader);

    reader.advance_read_pointer(10).unwrap();
    print_contiguous_buffer(&reader);

    write_to_contiguous_buffer(&mut writer, b"IJKLMN").unwrap();
    print_contiguous_buffer(&reader);

    reader.advance_read_pointer(4).unwrap();
    print_contiguous_buffer(&reader);

    println!("Loading 2 MFPs...");
    let mfps = MultiFragmentPacketRef::read_multiple(&mut reader, 2).unwrap();
    println!("Read MFP[0]: {:?}", mfps[0]);
    println!("Read MFP[1]: {:?}", mfps[1]);
    println!("Discarding MFPs...");
    mfps.discard().expect("Error discarding");
    println!("Loading 2 MFPs...");
    let mfps = MultiFragmentPacketRef::read_multiple(&mut reader, 2).unwrap();
    println!("Read MFP[0]: {:?}", mfps[0]);
    println!("Read MFP[1]: {:?}", mfps[1]);
}

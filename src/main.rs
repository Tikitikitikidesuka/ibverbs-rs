use env_logger::{Builder, Env};
use pcie40_rs::multi_fragment_packet::{MultiFragmentPacketBuilder, MultiFragmentPacketRef};
use pcie40_rs::pcie40_ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40_reader::PCIe40Reader;
use pcie40_rs::pcie40_stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40_rs::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40_stream::PCIe40StreamManager;
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
use std::io::{Read, stdin};

fn main() {
    let builder = MultiFragmentPacketBuilder::new()
        .with_align(2)
        .with_event_id(0)
        .with_fragment_version(0)
        .with_source_id(2)
        .lock_header();

    const DEVICE_NAME: &str = "tdtel202_0";

    Builder::from_env(Env::default().default_filter_or("trace"))
        .format_timestamp_secs()
        .format_file(true)
        .format_line_number(true)
        .init();

    let controller = PCIe40ControllerManager::open_by_device_name(DEVICE_NAME).unwrap();
    let meta_alignment = controller.meta_alignment().unwrap();

    let mut stream =
        PCIe40StreamManager::open_by_device_name(DEVICE_NAME, MainStream, MetaFormat).unwrap();
    stream
        .set_raii_enable_state_close_mode(PreserveEnableState)
        .unwrap();

    let mut stream_guard = stream.lock().unwrap();
    let mut reader = PCIe40Reader::new(stream_guard.map_buffer().unwrap(), meta_alignment).unwrap();
    //println!("\n\nDiscarding all data on the stream...\n\n");
    //reader.discard_all_data().unwrap();

    println!("\n\nStream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

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

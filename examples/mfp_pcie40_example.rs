use pcie40_rs::multi_fragment_packet::MultiFragmentPacketRef;
use pcie40_rs::pcie40::ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40::reader::PCIe40Reader;
use pcie40_rs::pcie40::stream::stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40_rs::pcie40::stream::stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40::stream::stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40::stream::stream::PCIe40StreamManager;
use pcie40_rs::typed_circular_buffer::CircularBufferMultiReadable;
use pcie40_rs::utils;
use pcie40_rs::utils::IsPow2Result;
use std::io::{Read, stdin};

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

    let mut locked_stream = stream.lock().unwrap();
    locked_stream.flush().unwrap();

    let mapped_stream = locked_stream.map_buffer().unwrap();

    let mut reader = PCIe40Reader::new(mapped_stream, meta_alignment_pow2).unwrap();

    println!("\n\nStream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

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
    println!("Discarding MFPs...");
    mfps.discard().expect("Error discarding");
}

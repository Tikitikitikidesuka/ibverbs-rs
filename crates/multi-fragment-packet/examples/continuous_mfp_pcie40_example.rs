use circular_buffer::CircularBufferMultiReadable;
use ebutils::IsPow2Result;
use multi_fragment_packet::MultiFragmentPacket;
use pcie40::ctrl::PCIe40ControllerManager;
use pcie40::reader::PCIe40Reader;
use pcie40::stream::stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40::stream::stream::PCIe40DAQStreamType::MainStream;
use pcie40::stream::stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40::stream::stream::PCIe40StreamManager;
use std::io::{Read, stdin};

fn main() {
    const DEVICE_NAME: &str = "tdtel203_1";

    let controller = PCIe40ControllerManager::open_by_device_name(DEVICE_NAME).unwrap();
    let meta_alignment_pow2 = match ebutils::pow2_exponent(controller.meta_alignment().unwrap()) {
        Some(pow2) => pow2,
        None => {
            panic!("Meta alignment is not a power of 2")
        }
    };

    let mut stream =
        PCIe40StreamManager::open_by_device_name(DEVICE_NAME, MainStream, MetaFormat).unwrap();
    stream
        .set_raii_enable_state_close_mode(PreserveEnableState)
        .unwrap();

    let mut locked_stream = stream.lock().unwrap();
    locked_stream.reset_flush().unwrap();
    locked_stream.reset_logic().unwrap();

    let mapped_stream = locked_stream.map_buffer().unwrap();

    let mut reader = PCIe40Reader::new(mapped_stream, meta_alignment_pow2).unwrap();

    println!("\n\nStream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    loop {
        println!("Loading 5 MFPs...");
        match MultiFragmentPacket::read_multiple(&mut reader, 5) {
            Ok(mfps) => {
                println!("Read MFP[0]: {:?}", mfps[0]);
                println!("Read MFP[1]: {:?}", mfps[1]);
                println!("Read MFP[2]: {:?}", mfps[2]);
                println!("Read MFP[3]: {:?}", mfps[3]);
                println!("Read MFP[4]: {:?}", mfps[4]);
                println!("Discarding MFPs...");
                mfps.discard().expect("Error discarding");
                println!("...\n");
            }
            Err(_) => {
                println!("Waiting for MFPs...")
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

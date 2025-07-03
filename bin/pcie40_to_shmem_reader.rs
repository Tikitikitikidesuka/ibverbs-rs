use pcie40_rs::multi_fragment_packet::MultiFragmentPacketRef;
use pcie40_rs::multi_fragment_packet::pcie40_readable::PCIe40TypedReadError;
use pcie40_rs::pcie40::ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40::reader::PCIe40Reader;
use pcie40_rs::pcie40::stream::stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40_rs::pcie40::stream::stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40::stream::stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40::stream::stream::PCIe40StreamManager;
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::writable_buffer_element::SharedMemoryTypedWriteError;
use pcie40_rs::shared_memory_buffer::writer::SharedMemoryBufferWriter;
use pcie40_rs::typed_circular_buffer::{CircularBufferMultiReadable, CircularBufferWritable};
use pcie40_rs::utils;
use pcie40_rs::utils::IsPow2Result;
use std::io::{Read, stdin};
use std::time::Duration;

fn main() {
    // -------------------------- //
    //    PCIe40 Stream Setup     //
    // -------------------------- //

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
    locked_stream.reset_flush().unwrap();
    locked_stream.reset_logic().unwrap();

    let mapped_stream = locked_stream.map_buffer().unwrap();
    let buffer_size = mapped_stream.size();

    let mut pcie40_reader = PCIe40Reader::new(mapped_stream, meta_alignment_pow2).unwrap();
    let buffer_alignment_pow2 = pcie40_reader.alignment_pow2();

    // -------------------------- //
    // Shared Memory Buffer Setup //
    // -------------------------- //

    let shmem_write_buffer =
        SharedMemoryBuffer::new_write_buffer("maredshemory33", buffer_size, buffer_alignment_pow2)
            .unwrap();

    let mut shmem_writer = SharedMemoryBufferWriter::new(shmem_write_buffer);

    // -------------------------- //
    //        READY TO GO!        //
    // -------------------------- //

    println!("\n\nStream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    loop {
        println!("Loading 5 MFPs...");

        // Wait for 5 MFPs to be ready
        pcie40_wait_for_mfps(&mut pcie40_reader, 5, Duration::from_millis(100))
            .expect("Error reading MFPs from shared memory");

        // Read the MFPs
        let mfps = MultiFragmentPacketRef::read_multiple(&mut pcie40_reader, 5)
            .expect("Error reading MFPs from shared memory");

        println!("Read MFP[0]: {:?}", mfps[0]);
        println!("Read MFP[1]: {:?}", mfps[1]);
        println!("Read MFP[2]: {:?}", mfps[2]);
        println!("Read MFP[3]: {:?}", mfps[3]);
        println!("Read MFP[4]: {:?}", mfps[4]);

        println!("Writing MFPs to shared memory...");

        shmem_write_mfps(&mut shmem_writer, &mfps, Duration::from_millis(100))
            .expect("Error writing MFPs to shared memory");

        println!("Wrote MFPs to shared memory successfully");

        println!("Discarding MFPs...");

        mfps.discard().unwrap();

        println!("Discarded MFPs successfully");

        println!("\n\n");
    }
}

fn pcie40_wait_for_mfps(
    reader: &mut PCIe40Reader,
    num: usize,
    poll_interval: Duration,
) -> Result<(), ()> {
    loop {
        match MultiFragmentPacketRef::read_multiple(reader, num) {
            Ok(_) => return Ok(()),
            Err(PCIe40TypedReadError::NotFound | PCIe40TypedReadError::NotEnoughData) => {
                println!("No MFPs found, waiting for more data...");
                std::thread::sleep(poll_interval);
            }
            Err(PCIe40TypedReadError::CorruptData | PCIe40TypedReadError::StreamError(_)) => {
                return Err(());
            }
        }
    }
}

fn shmem_write_mfps(
    writer: &mut SharedMemoryBufferWriter,
    mfps: &[&MultiFragmentPacketRef],
    poll_interval: Duration,
) -> Result<(), ()> {
    for mfp in mfps {
        loop {
            match mfp.write(writer) {
                Ok(_) => break, // Move to next MFP
                Err(error) => match error {
                    SharedMemoryTypedWriteError::NotEnoughSpace => {
                        println!("Temporary error writing MFP: {:?}, retrying...", error);
                        std::thread::sleep(poll_interval);
                    }
                },
            }
        }
    }
    Ok(())
}

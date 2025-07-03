use pcie40_rs::multi_fragment_packet::MultiFragmentPacketRef;
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::readable_buffer_element::SharedMemoryTypedReadError;
use pcie40_rs::shared_memory_buffer::reader::SharedMemoryBufferReader;
use pcie40_rs::typed_circular_buffer::CircularBufferMultiReadable;
use pcie40_rs::typed_circular_buffer_read_guard::MultiReadGuard;
use std::io::{Read, stdin};
use std::time::Duration;

fn main() {
    // -------------------------- //
    // Shared Memory Buffer Setup //
    // -------------------------- //

    let read_buffer = SharedMemoryBuffer::new_read_buffer("maredshemory33").unwrap();
    let mut reader = SharedMemoryBufferReader::new(read_buffer);

    println!("\n\nStream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    // -------------------------- //
    //        READY TO GO!        //
    // -------------------------- //

    loop {
        println!("Receiving MFPs from shared memory...");

        let mfps = shmem_read_mfps(&mut reader, 5, Duration::from_millis(100))
            .expect("Error reading MFPs from PCIe40");

        println!("Read MFP[0]: {:?}", mfps[0]);
        println!("Read MFP[1]: {:?}", mfps[1]);
        println!("Read MFP[2]: {:?}", mfps[2]);
        println!("Read MFP[3]: {:?}", mfps[3]);
        println!("Read MFP[4]: {:?}", mfps[4]);

        println!("Discarding MFPs...");

        mfps.discard().unwrap();

        println!("Discarded MFPs successfully");

        println!("\n\n");
    }
}

fn shmem_read_mfps(
    reader: &mut SharedMemoryBufferReader,
    num: usize,
    poll_interval: Duration,
) -> Result<MultiReadGuard<SharedMemoryBufferReader, MultiFragmentPacketRef>, ()> {
    loop {
        match MultiFragmentPacketRef::read_multiple(reader, num) {
            Ok(guard) => return Ok(guard),
            Err(
                SharedMemoryTypedReadError::NotFound | SharedMemoryTypedReadError::NotEnoughData,
            ) => {
                println!("No MFPs found, waiting for more data...");
                std::thread::sleep(poll_interval);
            }
            Err(SharedMemoryTypedReadError::CorruptData) => {
                return Err(());
            }
        }
    }
}

use pcie40_rs::multi_fragment_packet::MultiFragmentPacketRef;
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::readable_buffer_element::SharedMemoryTypedReadError;
use pcie40_rs::shared_memory_buffer::reader::SharedMemoryBufferReader;
use pcie40_rs::typed_circular_buffer::CircularBufferMultiReadable;
use std::env;
use std::io::{Read, stdin};
use std::time::Duration;

fn main() {
    // -------------------------- //
    // Shared Memory Buffer Setup //
    // -------------------------- //

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <shmem_name>", args[0]);
        std::process::exit(1);
    }

    let shmem_name = &args[1];
    let read_buffer = SharedMemoryBuffer::new_read_buffer(shmem_name).unwrap();
    let shmem_buffer_size = read_buffer.size();

    let mut reader = SharedMemoryBufferReader::new(read_buffer);

    println!(
        "\n\nGot shared memory buffer of size: {}",
        shmem_buffer_size
    );
    println!("Stream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    // -------------------------- //
    //        READY TO GO!        //
    // -------------------------- //

    loop {
        println!("Receiving MFPs from shared memory...");

        // Wait for 5 MFPs to be ready
        shmem_wait_for_mfps(&mut reader, 5, Duration::from_millis(100))
            .expect("Error reading MFPs from shared memory");

        // Read the MFPs
        let mfps = MultiFragmentPacketRef::read_multiple(&mut reader, 5)
            .expect("Error reading MFPs from shared memory");

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

// This hits a limitation on the borrow checker that cannot realize that when returning the guard,
// the next iteration will not occur and therefore this is safe. Many people have this problem.
// It can be solved in nightly with alpha Polonius
/*
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
                continue;
            }
            Err(SharedMemoryTypedReadError::CorruptData) => {
                return Err(());
            }
        }
    }
}
*/

// For now we solve it by just waiting for one to be ready and then the user has to read it again.
fn shmem_wait_for_mfps(
    reader: &mut SharedMemoryBufferReader,
    num: usize,
    poll_interval: Duration,
) -> Result<(), ()> {
    loop {
        match MultiFragmentPacketRef::read_multiple(reader, num) {
            Ok(_) => return Ok(()),
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

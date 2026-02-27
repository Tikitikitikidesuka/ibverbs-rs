use circular_buffer::CircularBufferMultiReadable;
use multi_fragment_packet::MultiFragmentPacket;
use shared_memory_buffer::{SharedMemoryBufferReader, SharedMemoryTypedReadError};
use std::env;
use std::io::{Read, stdin};
use std::time::Duration;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // -------------------------- //
    // Shared Memory Buffer Setup //
    // -------------------------- //

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <shmem_name>", args[0]);
        std::process::exit(1);
    }

    let shmem_name = &args[1];
    let mut reader = SharedMemoryBufferReader::open(shmem_name).unwrap();
    let shmem_buffer_size = reader.buffer_size();

    println!("\n\nGot shared memory buffer of size: {shmem_buffer_size}");
    println!("Stream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    // -------------------------- //
    //        READY TO GO!        //
    // -------------------------- //

    let mut last_event = 0;

    loop {
        println!("Receiving MFPs from shared memory...");

        // Wait for 5 MFPs to be ready
        shmem_wait_for_mfps(&mut reader, 5, Duration::from_millis(100))
            .expect("Error reading MFPs from shared memory");

        // Read the MFPs
        let mfps = MultiFragmentPacket::read_multiple(&mut reader, 5)
            .expect("Error reading MFPs from shared memory");

        // Check MFPs follow proper order
        let local_first_event = mfps[0].event_id();
        let local_last_event = mfps[4].event_id() + u64::from(mfps[4].fragment_count());
        let local_num_events = mfps.iter().fold(0, |acc, x| acc + x.fragment_count());
        assert_eq!(last_event, local_first_event);
        assert_eq!(
            local_last_event - local_first_event,
            u64::from(local_num_events)
        );
        last_event = local_last_event;

        println!("Read MFP[0]: {:?}", mfps[0]);
        println!("Read MFP[1]: {:?}", mfps[1]);
        println!("Read MFP[2]: {:?}", mfps[2]);
        println!("Read MFP[3]: {:?}", mfps[3]);
        println!("Read MFP[4]: {:?}", mfps[4]);

        println!("Discarding MFPs...");

        mfps.discard_all().unwrap();

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
) -> Result<MultiReadGuard<SharedMemoryBufferReader, MultiFragmentPacket>, ()> {
    loop {
        match MultiFragmentPacket::read_multiple(reader, num) {
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
        match MultiFragmentPacket::read_multiple(reader, num) {
            Ok(_) => return Ok(()),
            Err(SharedMemoryTypedReadError::NotEnoughData) => {
                println!("No MFPs found, waiting for more data...");
                std::thread::sleep(poll_interval);
            }
            Err(SharedMemoryTypedReadError::CorruptData) => {
                return Err(());
            }
        }
    }
}

use pcie40_rs::multi_fragment_packet::{Fragment, MultiFragmentPacket, MultiFragmentPacketBuilder};
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::writer::SharedMemoryBufferWriter;
use pcie40_rs::typed_circular_buffer::CircularBufferWritable;
use std::env;
use std::io::{Read, stdin};
use std::time::Duration;

fn main() {
    const BUFFER_SIZE: usize = 1 << 32; // 4Gb
    const ALIGNMENT_POW2: u8 = 12;

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
    let shmem_write_buffer =
        SharedMemoryBuffer::new_write_buffer(shmem_name, BUFFER_SIZE, ALIGNMENT_POW2).unwrap();
    let shmem_buffer_size = shmem_write_buffer.size();

    let mut shmem_writer = SharedMemoryBufferWriter::new(shmem_write_buffer);

    // -------------------------- //
    //        READY TO GO!        //
    // -------------------------- //

    println!(
        "\n\nGot shared memory buffer of size: {}",
        shmem_buffer_size
    );
    println!("Stream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    let mut event_id = 0;

    loop {
        println!("Loading 5 MFPs...");

        // Mock read 5 MFPs
        let mut mfps = Vec::with_capacity(5);
        for _ in 0..5 {
            let mfp = MultiFragmentPacketBuilder::new()
                .with_fragment_version(1)
                .with_source_id(1)
                .with_align(6)
                .with_event_id(event_id)
                .lock_header()
                .add_fragments(
                    (0..1000).map(|_| Fragment::new(1, (0..255).collect::<Vec<_>>()).unwrap()),
                )
                .build();
            mfps.push(mfp);
            event_id += 1000;
        }

        println!("Read MFP[0]: {:?}", mfps[0].as_ref());
        println!("Read MFP[1]: {:?}", mfps[1].as_ref());
        println!("Read MFP[2]: {:?}", mfps[2].as_ref());
        println!("Read MFP[3]: {:?}", mfps[3].as_ref());
        println!("Read MFP[4]: {:?}", mfps[4].as_ref());

        println!("Writing MFPs to shared memory...");

        shmem_write_mfps(
            &mut shmem_writer,
            mfps.as_slice(),
            Duration::from_millis(100),
        )
        .expect("Error writing MFPs to shared memory");

        println!("Wrote MFPs to shared memory successfully");

        println!("Discarding MFPs...");

        // Mock discard 5 MFPs (does nothing)

        println!("Discarded MFPs successfully");

        println!("\n\n");
    }
}

fn shmem_write_mfps(
    writer: &mut SharedMemoryBufferWriter,
    mfps: &[MultiFragmentPacket],
    poll_interval: Duration,
) -> Result<(), ()> {
    for mfp in mfps {
        loop {
            match mfp.write(writer) {
                Ok(_) => break, // Move to next MFP
                Err(error) => match error {
                    _ => {
                        println!("Temporary error writing MFP: {:?}, retrying...", error);
                        std::thread::sleep(poll_interval);
                    }
                },
            }
        }
    }
    Ok(())
}

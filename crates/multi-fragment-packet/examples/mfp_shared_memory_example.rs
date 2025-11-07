use circular_buffer::{
    CircularBufferMultiReadable, CircularBufferReadable, CircularBufferWritable,
};
use multi_fragment_packet::{MultiFragmentPacketBuilder, MultiFragmentPacketRef};
use shared_memory_buffer::{
    SharedMemoryBuffer, SharedMemoryBufferReader, SharedMemoryBufferWriter,
};

fn main() {
    // Create the buffer with size 1024 bytes, alignment 8 (2^8 = 256 bytes) (max 4 elements of 256 bytes)
    let write_buffer = SharedMemoryBuffer::new_write_buffer("maredshemory33", 1024, 8).unwrap();
    let read_buffer = SharedMemoryBuffer::new_read_buffer("maredshemory33").unwrap();

    // Create reader and writer
    let mut reader = SharedMemoryBufferReader::new(read_buffer);
    let mut writer = SharedMemoryBufferWriter::new(write_buffer);

    println!("Ready!!!");

    // [0, , , ]
    println!("Writing MFP 0 to shmem...");
    let mfp_0_256 = MultiFragmentPacketBuilder::new()
        .with_align_log(4)
        .with_event_id(0)
        .with_source_id(1)
        .with_fragment_version(1)
        .add_fragment(1, (0..190).collect::<Vec<_>>())
        .build();
    mfp_0_256.write(&mut writer).unwrap();
    println!(
        "Done! Size on buffer: {}",
        alignment_utils::align_up_pow2(mfp_0_256.packet_size() as usize, writer.alignment_pow2())
    );

    // [0,1, , ]
    // Writable is also implemented for the buffered entry so one can be
    // read and written again without copying it out of the buffer
    println!("Writing MFP 0 again to shmem...");
    let read_mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    read_mfp.write(&mut writer).unwrap();
    println!(
        "Done! Size on buffer: {}",
        alignment_utils::align_up_pow2(read_mfp.packet_size() as usize, writer.alignment_pow2())
    );

    // [0,1,2, ]
    println!("Writing MFP 2 to shmem...");
    let mfp_2_256 = MultiFragmentPacketBuilder::new()
        .with_align_log(4)
        .with_event_id(2)
        .with_source_id(1)
        .with_fragment_version(1)
        .add_fragment(1, (40..255).collect::<Vec<_>>())
        .build();
    mfp_2_256.write(&mut writer).unwrap();
    println!(
        "Done! Size on buffer: {}",
        alignment_utils::align_up_pow2(mfp_2_256.packet_size() as usize, writer.alignment_pow2())
    );

    // [ ,1,2, ]
    println!("Reading first instance of MFP 0 from shmem...");
    let read_mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    println!("Read: {}", *read_mfp);
    println!("Discarding it...");
    read_mfp.discard().unwrap();

    // [ , ,2, ]
    println!("Reading second instance of MFP 0 from shmem...");
    let read_mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    println!("Read: {}", *read_mfp);
    println!("Discarding it...");
    read_mfp.discard().unwrap();

    // [3,3,2,W]
    println!("Writing MFP 3 to shmem (this one sholuld trigger wrap behaviour)...");
    let mfp_3_512 = MultiFragmentPacketBuilder::new()
        .with_align_log(4)
        .with_event_id(3)
        .with_source_id(1)
        .with_fragment_version(1)
        .add_fragment(1, (0..255).collect::<Vec<_>>())
        .build();
    mfp_3_512.write(&mut writer).unwrap();
    println!(
        "Done! Size on buffer: {}",
        alignment_utils::align_up_pow2(mfp_3_512.packet_size() as usize, writer.alignment_pow2())
    );

    // [ , , , ]
    println!("Reading MFPs 0 and 3 from shmem...");
    let read_entries = MultiFragmentPacketRef::read_multiple(&mut reader, 2).unwrap();
    read_entries.iter().for_each(|entry| {
        println!("Read many: {}", entry);
    });
    println!("Discarding them...");
    read_entries.discard().unwrap();
}

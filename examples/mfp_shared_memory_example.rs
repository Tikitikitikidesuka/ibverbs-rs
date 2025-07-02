use pcie40_rs::multi_fragment_packet::{
    Fragment, MultiFragmentPacketBuilder, MultiFragmentPacketRef,
};
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::reader::SharedMemoryBufferReader;
use pcie40_rs::shared_memory_buffer::writer::SharedMemoryBufferWriter;
use pcie40_rs::typed_circular_buffer::{
    CircularBufferMultiReadable, CircularBufferReadable, CircularBufferWritable,
};
use pcie40_rs::utils;

fn main() {
    // Create the buffer with size 1024 bytes, alignment 8 (2^8 = 256 bytes) (max 4 elements of 256 bytes)
    let write_buffer = SharedMemoryBuffer::new_write_buffer("maredshemory33", 1024, 8).unwrap();
    let read_buffer = SharedMemoryBuffer::new_read_buffer("maredshemory33").unwrap();

    // Create reader and writer
    let mut reader = SharedMemoryBufferReader::new(read_buffer);
    let mut writer = SharedMemoryBufferWriter::new(write_buffer);

    // [0, , , ]
    let mfp_0_256 = MultiFragmentPacketBuilder::new()
        .with_align(4)
        .with_event_id(0)
        .with_source_id(1)
        .with_fragment_version(1)
        .lock_header()
        .add_fragment(Fragment::new(1, (0..190).collect::<Vec<_>>()).unwrap())
        .build();
    mfp_0_256.write(&mut writer).unwrap();
    println!(
        "Size: {}",
        utils::align_up_pow2(mfp_0_256.packet_size() as usize, writer.alignment_pow2())
    );

    // [0,1, , ]
    // Writable is also implemented for the buffered entry so one can be
    // read and written again without copying it out of the buffer
    let read_mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    read_mfp.write(&mut writer).unwrap();
    println!("Same one");

    // [0,1,2, ]
    let mfp_2_256 = MultiFragmentPacketBuilder::new()
        .with_align(4)
        .with_event_id(2)
        .with_source_id(1)
        .with_fragment_version(1)
        .lock_header()
        .add_fragment(Fragment::new(1, (40..255).collect::<Vec<_>>()).unwrap())
        .build();
    mfp_2_256.write(&mut writer).unwrap();
    println!(
        "Size: {}",
        utils::align_up_pow2(mfp_2_256.packet_size() as usize, writer.alignment_pow2())
    );

    // [ ,1,2, ]
    let read_mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    println!("Consume: {}", *read_mfp);
    read_mfp.discard().unwrap();

    // [ , ,2, ]
    let read_entry = MultiFragmentPacketRef::read(&mut reader).unwrap();
    println!("Consume: {}", *read_entry);
    read_entry.discard().unwrap();

    // [3,3,2,W]
    let mfp_3_512 = MultiFragmentPacketBuilder::new()
        .with_align(4)
        .with_event_id(3)
        .with_source_id(1)
        .with_fragment_version(1)
        .lock_header()
        .add_fragment(Fragment::new(1, (0..255).collect::<Vec<_>>()).unwrap())
        .build();
    mfp_3_512.write(&mut writer).unwrap();
    println!(
        "Size: {}",
        utils::align_up_pow2(mfp_3_512.packet_size() as usize, writer.alignment_pow2())
    );

    // [ , , , ]
    let read_entries = MultiFragmentPacketRef::read_multiple(&mut reader, 2).unwrap();
    read_entries.iter().for_each(|entry| {
        println!("Read many: {}", entry);
    });
    read_entries.discard().unwrap();
}

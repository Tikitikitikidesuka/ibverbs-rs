use log::LevelFilter;
use pcie40_rs::multi_fragment_packet::{
    Fragment, MultiFragmentPacketBuilder, MultiFragmentPacketRef,
};
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::reader::SharedMemoryBufferReader;
use pcie40_rs::shared_memory_buffer::writer::SharedMemoryBufferWriter;
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
use pcie40_rs::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .default_format()
        .format_line_number(true)
        .init();

    let write_buffer = SharedMemoryBuffer::new_write_buffer("keo_33", 128, 3).unwrap();
    let mut writer = SharedMemoryBufferWriter::new(write_buffer);

    let read_buffer = SharedMemoryBuffer::new_read_buffer("keo_33").unwrap();
    let mut reader = SharedMemoryBufferReader::new(read_buffer);

    //let result = writer.write(&[10, 20, 30]);
    //if let Err(err) = result {
    //println!("Failed to write: {}", err);
    //}

    writer.write(&[1, 2, 3, 4]).unwrap();

    reader.load_all_data().unwrap();
    let data = reader.data();
    println!("{:?}", data.data_ref());
    data.discard().unwrap();

    writer.write(&[5, 6, 7, 8, 9, 10, 11, 12]).unwrap();

    println!("READING SECOND TIME\n\n\n\n\n\n");
    reader.load_all_data().unwrap();
    let data = reader.data();
    println!("{:?}", data.data_ref());
    data.discard().unwrap();

    println!("READING THIRD TIME\n\n\n\n\n\n");
    let data = reader.data();
    println!("{:?}", data.data_ref());
    data.discard().unwrap();

    let mfp = MultiFragmentPacketBuilder::new()
        .with_event_id(1)
        .with_source_id(33)
        .with_align(3)
        .with_fragment_version(3)
        .lock_header()
        .add_fragment(Fragment::new(1, [1, 2, 3]).unwrap())
        .add_fragment(Fragment::new(2, [4, 5, 6]).unwrap())
        .build();

    println!("WRITING MFP\n\n\n\n\n\n");
    writer.write(mfp.raw_packet_data()).unwrap();
    writer.write(&[0, 0, 0, 0]).unwrap();

    println!("READING MFP\n\n\n\n\n\n");
    let read_mfp = MultiFragmentPacketRef::read(&mut reader).unwrap();
    println!("MFP: {:?}", read_mfp);
}

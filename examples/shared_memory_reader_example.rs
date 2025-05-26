use log::LevelFilter;
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::reader::SharedMemoryBufferReader;
use pcie40_rs::shared_memory_buffer::writer::SharedMemoryBufferWriter;
use pcie40_rs::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .default_format()
        .format_line_number(true)
        .init();

    let write_buffer = SharedMemoryBuffer::new_write_buffer("keo_33", 8, 3).unwrap();
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
}

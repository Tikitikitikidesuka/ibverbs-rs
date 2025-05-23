use log::LevelFilter;
use pcie40_rs::shared_memory_buffer::buffer_backend::SharedMemoryBuffer;
use pcie40_rs::shared_memory_buffer::writer::SharedMemoryBufferWriter;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .init();

    let write_buffer = SharedMemoryBuffer::new_write_buffer("keo_33", 1024, 3).unwrap();
    let mut writer = SharedMemoryBufferWriter::new(write_buffer);

    writer.write(&[0, 1, 2, 3, 4]).unwrap();
}
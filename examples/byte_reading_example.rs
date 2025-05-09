use crate::example_reader::ExampleReader;
use env_logger::{Builder, Env};
use pcie40_rs::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

#[path = "../examples/example_reader.rs"]
mod example_reader;

fn main() {
    // Set filter to "trace" for maximum logging detail
    Builder::from_env(Env::default().default_filter_or("warn"))
        .format_timestamp_secs()
        .format_file(true)
        .format_line_number(true)
        .init();

    let demo_data: Vec<u8> = (0..128).collect();

    println!("Instantiating example reader with demo data...");
    let mut reader = ExampleReader::new(demo_data, 64);
    println!();

    println!("Loading data...");
    let loaded_byte_num = reader.load_all_data().unwrap();
    println!("Loaded {} bytes", loaded_byte_num);
    println!("Guarded data: {:?}", reader.data());
    println!();

    println!("Discarding 16 bytes...");
    let discarded_bytes = reader.discard_data(16).unwrap();
    println!("Discarded {} bytes", discarded_bytes);
    println!("Guarded data: {:?}", reader.data());
    println!();

    println!("Discarding another 64 bytes...");
    let discarded_bytes = reader.discard_data(16).unwrap();
    // Notice only 48 bytes will be discarded since there are only 48 loaded bytes
    println!("Discarded {} bytes", discarded_bytes);
    println!("Guarded data: {:?}", reader.data());
    println!();

    println!("Loading again...");
    let loaded_byte_num = reader.load_all_data().unwrap();
    println!("Loaded {} bytes", loaded_byte_num);
    println!("Guarded data: {:?}", reader.data());
    println!();

    println!("Discarding all loaded data...");
}

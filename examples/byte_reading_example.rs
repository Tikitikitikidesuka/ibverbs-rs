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

    println!("Loading 32 bytes...");
    let loaded_byte_num = reader.load_data(32).unwrap();
    println!("Loaded {} bytes", loaded_byte_num);
    println!("Guarded data: {:?}", reader.data());
    println!();

    println!("Loading another 64 bytes...");
    let loaded_byte_num = reader.load_data(64).unwrap();
    // Notice only 32 bytes will be loaded since the write pointer of the
    // demo reader makes the write pointer be at 64 bytes form the read pointer
    println!("Loaded {} bytes: {:?}", loaded_byte_num, reader.data());
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

    println!("Loading another 64 bytes...");
    let loaded_byte_num = reader.load_data(64).unwrap();
    println!("Loaded {} bytes: {:?}", loaded_byte_num, reader.data());
    println!("Guarded data: {:?}", reader.data());
    println!();

    println!("Discarding all loaded data...");

    /*
    let first_i32list = I32ListRef::read(&mut reader).unwrap();
    println!("Read: {}", first_i32list);
    println!();

    println!("\nLoading three I32ListRefs...");
    // Expect to fail because of max 64 bytes loaded on the demo reader
    match I32ListRef::read_multiple(&mut reader, 3) {
        Err(error) => println!("Failed to read three I32ListRefs: {:?}", error),
        Ok(i32_list) => println!("Read: {}", i32_list),
    };
    println!();

    println!("Loading two I32ListRefs...");
    // Expect to succeed because this does not exceed the max 64 bytes loaded on the demo reader
    let first_and_second_i32_lists = I32ListRef::read_multiple(&mut reader, 2)
        .map_err(|error| println!("Failed to read three I32ListRefs: {:?}", error))
        .unwrap();
    println!("Read: {}", first_and_second_i32_lists);
    // Notice the first I32ListRef of the two loaded is the same as the first one loaded individually
    // Data is not discarded unless explicitly ordered to
    println!();

    println!("Discarding the first two I32ListRefs...");
    first_and_second_i32_lists.discard().unwrap();
    println!();

    println!("Loading another two I32ListRefs...");
    // Expect to succeed because this does not exceed the max 64 bytes loaded on the demo reader
    let second_and_third = I32ListRef::read_multiple(&mut reader, 2)
        .map_err(|error| println!("Failed to read three I32ListRefs: {:?}", error))
        .unwrap();
    println!("Read: {}", second_and_third);
    // Notice the data is different this time around
    // This time the previous read data was discarded before reading again
    println!();
     */
}

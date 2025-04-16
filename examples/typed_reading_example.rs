#[path = "../examples/example_readable.rs"]
mod example_readable;

#[path = "../examples/example_reader.rs"]
mod example_reader;

use crate::example_readable::I32ListRef;
use crate::example_reader::ExampleReader;
use env_logger::{Builder, Env};
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;

fn main() {
    // Set filter to "trace" for maximum logging detail
    Builder::from_env(Env::default().default_filter_or("warn"))
        .format_timestamp_secs()
        .format_file(true)
        .format_line_number(true)
        .init();

    let demo_data: Vec<u8> = [
        // I32List(id=0, count=4, elements=[0, 1, 2, 3])
        vec![0, 4, 0, 1, 2, 3],
        // I32List(id=1, count=5, elements=[4, 5, 6, 7, 8])
        vec![1, 5, 4, 5, 6, 7, 8],
        // I32List(id=2, count=3, elements=[9, 10, 11])
        vec![2, 3, 9, 10, 11],
        // I32List(id=3, count=9, elements=[12, 13, 14, 15, 16, 17, 18, 19, 20])
        vec![3, 9, 12, 13, 14, 15, 16, 17, 18, 19, 20],
    ]
    .concat()
    .iter()
    .flat_map(|value: &i32| value.to_le_bytes())
    .collect();

    println!("Instantiating example reader with demo data...");
    let mut reader = ExampleReader::new(demo_data, 64);
    println!();

    println!("Reading an I32ListRef...");
    let first_i32list = I32ListRef::read(&mut reader).unwrap();
    println!("Read: {}", first_i32list);
    println!();

    println!("\nLoading three I32ListRefs...");
    // Expect to fail because of max 64 bytes loaded on the demo reader
    // TODO: ADD A BLOCKING VERSION OF READ AND READ_MULTIPLE
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
}

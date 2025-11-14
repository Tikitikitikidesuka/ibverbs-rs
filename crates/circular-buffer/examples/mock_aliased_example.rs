use circular_buffer::mock_buffers::{
    MockAliasedBuffer, MockAliasedBufferReader, MockAliasedBufferWriter,
};
use circular_buffer::{CircularBufferReader, CircularBufferWriter};

fn main() {
    // Create the buffer with size 16 bytes, alignment 1 (2^1 = 2 bytes)
    let mut demo_buffer = MockAliasedBuffer::new(16, 1).unwrap();

    // Create reader and writer
    let mut reader = MockAliasedBufferReader::new(&mut demo_buffer).unwrap();
    let mut writer = MockAliasedBufferWriter::new(&mut demo_buffer).unwrap();

    write_to_contiguous_buffer(&mut writer, b"0123456789ABCD").unwrap();
    print_contiguous_buffer(&reader);

    reader.advance_read_pointer(2).unwrap();
    print_contiguous_buffer(&reader);

    write_to_contiguous_buffer(&mut writer, b"EFGH").unwrap();
    print_contiguous_buffer(&reader);

    reader.advance_read_pointer(10).unwrap();
    print_contiguous_buffer(&reader);

    write_to_contiguous_buffer(&mut writer, b"IJKLMN").unwrap();
    print_contiguous_buffer(&reader);

    reader.advance_read_pointer(4).unwrap();
    print_contiguous_buffer(&reader);
}

fn write_to_contiguous_buffer(writer: &mut MockAliasedBufferWriter, data: &[u8]) -> Result<(), ()> {
    let writable_region = writer.writable_region();

    if data.len() > writable_region.len() {
        Err(())
    } else {
        writable_region[..data.len()].copy_from_slice(data);
        writer.advance_write_pointer(data.len()).map_err(|_| ())?;
        Ok(())
    }
}

fn print_contiguous_buffer(reader: &MockAliasedBufferReader) {
    let readable_region = reader.readable_region();

    println!("\nREAD: ");
    println!("Region: {:?}", readable_region);
}

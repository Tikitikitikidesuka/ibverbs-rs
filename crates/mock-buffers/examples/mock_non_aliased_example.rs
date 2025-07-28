use circular_buffer::{CircularBufferReader, CircularBufferWriter};
use mock_buffers::non_aliased_buffer::{MockNonAliasedBuffer, MockNonAliasedBufferReader, MockNonAliasedBufferWriter};

fn main() {
    // Create the buffer with size 16 bytes, alignment 1 (2^1 = 2 bytes)
    let mut demo_buffer = MockNonAliasedBuffer::new(16, 1).unwrap();

    // Create reader and writer
    let mut reader = MockNonAliasedBufferReader::new(&mut demo_buffer);
    let mut writer = MockNonAliasedBufferWriter::new(&mut demo_buffer);

    write_to_non_contiguous_buffer(&mut writer, b"0123456789ABCD").unwrap();
    print_non_contiguous_buffer(&reader);

    reader.advance_read_pointer(2).unwrap();
    print_non_contiguous_buffer(&reader);

    write_to_non_contiguous_buffer(&mut writer, b"EFGH").unwrap();
    print_non_contiguous_buffer(&reader);

    reader.advance_read_pointer(10).unwrap();
    print_non_contiguous_buffer(&reader);

    write_to_non_contiguous_buffer(&mut writer, b"IJKLMN").unwrap();
    print_non_contiguous_buffer(&reader);

    reader.advance_read_pointer(4).unwrap();
    print_non_contiguous_buffer(&reader);
}

fn write_to_non_contiguous_buffer(
    writer: &mut MockNonAliasedBufferWriter,
    data: &[u8],
) -> Result<(), ()> {
    let (primary_region, secondary_region) = writer.writable_region();

    if data.len() > primary_region.len() + secondary_region.len() {
        Err(())
    } else if data.len() > primary_region.len() {
        let secondary_data_len = data.len() - primary_region.len();

        primary_region.copy_from_slice(&data[..primary_region.len()]);
        secondary_region[..secondary_data_len].copy_from_slice(&data[primary_region.len()..]);

        writer.advance_write_pointer(data.len()).map_err(|_| ())?;

        Ok(())
    } else {
        primary_region[..data.len()].copy_from_slice(&data);

        writer.advance_write_pointer(data.len()).map_err(|_| ())?;

        Ok(())
    }
}

fn print_non_contiguous_buffer(reader: &MockNonAliasedBufferReader) {
    let (primary_region, secondary_region) = reader.readable_region();

    println!("\nREAD: ");
    println!("Primary: {:?}", primary_region);
    println!("Secondary: {:?}", secondary_region);
}

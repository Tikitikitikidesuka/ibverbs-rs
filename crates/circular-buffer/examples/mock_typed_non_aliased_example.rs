use circular_buffer::mock_buffers::{
    BufferedDiaryEntry, MockNonAliasedBuffer, MockNonAliasedBufferReader,
    MockNonAliasedBufferWriter, MockWritable, OwnedDiaryEntry,
};
use circular_buffer::{CircularBufferReadable, CircularBufferWritable, ReadGuard};

fn main() {
    // [ , , , ]
    let mut demo_buffer = MockNonAliasedBuffer::new(128, 5).unwrap();

    let mut reader = MockNonAliasedBufferReader::new(&mut demo_buffer).unwrap();
    let mut writer = MockNonAliasedBufferWriter::new(&mut demo_buffer).unwrap();

    // [0, , , ]
    let writable_entry_0_32 = OwnedDiaryEntry::new(1, 1, 2000, "First B)".to_string());
    writable_entry_0_32.write(&mut writer).unwrap();
    println!(
        "Size: {}",
        ebutils::align_up_pow2(writable_entry_0_32.buffered_size(), writer.alignment_pow2())
    );

    // [0,1, , ]
    // Writable is also implemented for the buffered entry so one can be
    // read and written again without copying it out of the buffer
    let read_entry = BufferedDiaryEntry::read(&mut reader, 1).unwrap();
    read_entry[0].write(&mut writer).unwrap();

    // [0,1,2, ]
    let writable_entry_2_32 = OwnedDiaryEntry::new(3, 3, 2000, "Third!?! 0_0".to_string());
    writable_entry_2_32.write(&mut writer).unwrap();
    println!(
        "Size: {}",
        ebutils::align_up_pow2(writable_entry_2_32.buffered_size(), writer.alignment_pow2())
    );

    // [ ,1,2, ]
    let read_entry = BufferedDiaryEntry::read(&mut reader, 1).unwrap();
    println!("Consume: {}", read_entry[0]);
    read_entry.discard().unwrap();

    // [ , ,2, ]
    let read_entry = BufferedDiaryEntry::read(&mut reader, 1).unwrap();
    println!("Consume: {}", read_entry[0]);
    read_entry.discard().unwrap();

    // [3,3,2,W]
    let writable_entry_3_64 =
        OwnedDiaryEntry::new(4, 4, 2000, "FOURTH!!?!? IS THERE NO END?! ;-;".to_string());
    writable_entry_3_64.write(&mut writer).unwrap();
    println!(
        "Size: {}",
        ebutils::align_up_pow2(writable_entry_3_64.buffered_size(), writer.alignment_pow2())
    );

    // [ , , , ]
    let read_entries = BufferedDiaryEntry::read(&mut reader, 2).unwrap();
    read_entries.iter().for_each(|entry| {
        println!("Read many: {}", entry);
    });
    read_entries.discard().unwrap();
}

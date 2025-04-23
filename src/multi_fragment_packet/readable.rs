use crate::multi_fragment_packet::{
    HEADER_SIZE, MultiFragmentPacketFromRawBytesError, MultiFragmentPacketRef,
};
use crate::typed_zero_copy_ring_buffer_reader::{
    CastBytesRef, ZeroCopyRingBufferReadable, ZeroCopyRingBufferReadableError,
    ensure_available_bytes,
};
use crate::utils;
use crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

impl<R> ZeroCopyRingBufferReadable<'_, R> for MultiFragmentPacketRef
where
    R: ZeroCopyRingBufferReader,
{
    fn load(reader: &mut R, offset: usize) -> Result<usize, ZeroCopyRingBufferReadableError> {
        // Ensure enough data for the header
        ensure_available_bytes(reader, offset + HEADER_SIZE)?;

        // Get temporary access to the data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[offset..(offset + HEADER_SIZE)];
        let mfp = unsafe { &*(header_data.as_ptr() as *const MultiFragmentPacketRef) };

        // Get the total packet size from the header
        let packet_size = mfp.packet_size() as usize;

        let (aligned_size, aligned_load) = if let Some(alignment) =
            reader.alignment().map_err(|error| {
                ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
            })? {
            (
                utils::align_up(packet_size, alignment),
                utils::align_up(offset + packet_size, alignment),
            )
        } else {
            (packet_size, offset + packet_size)
        };

        // Ensure enough data for the whole mfp
        ensure_available_bytes(reader, aligned_load)?;

        Ok(aligned_size)
    }
}

impl CastBytesRef for MultiFragmentPacketRef {
    fn cast(data: &[u8]) -> Result<&Self, ZeroCopyRingBufferReadableError> {
        MultiFragmentPacketRef::ref_from_raw_bytes(data).map_err(|error| match error {
            MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                required_data: required_bytes,
                available_data: available_bytes,
            } => ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                required_data: required_bytes,
                available_data: available_bytes,
            },
            MultiFragmentPacketFromRawBytesError::CorruptedMagic {
                read_magic,
                expected_magic,
            } => ZeroCopyRingBufferReadableError::ImproperlyFormattedData {
                message: format!(
                    "Expected magic bytes {:x?} but found {:x?}",
                    expected_magic, read_magic
                ),
            },
        })
    }
}

/*
#[cfg(test)]
mod tests {
    use crate::multi_fragment_packet::{MultiFragmentPacketBuilder, MultiFragmentPacketRef, Fragment};
    use crate::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
    use crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;
    use crate::mock_reader::MockReader;

    fn create_test_mfp() -> Vec<u8> {
        MultiFragmentPacketBuilder::new()
            .with_magic(0x40CE)
            .with_event_id(1)
            .with_source_id(1)
            .with_align(2)
            .with_fragment_version(1)
            .lock_header()
            .add_fragment(Fragment::new(0, vec![0, 1, 2, 3]).unwrap())
            .add_fragment(Fragment::new(1, vec![4, 5, 6, 7, 8]).unwrap())
            .build()
            .raw_packet_data()
            .to_vec()
    }

    // Helper to extend the MockReader implementation with alignment support
    impl MockReader {
        fn with_alignment(data: Vec<u8>, write_offset: usize) -> Self {
            // Only use the provided method from MockReader
            MockReader::new(data, write_offset)
        }

        fn alignment(&self) -> Result<Option<usize>, crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReaderError> {
            // Default to 4 byte alignment for tests
            Ok(Some(4))
        }
    }

    #[test]
    fn test_load_success() {
        let mfp_data = create_test_mfp();
        // Use write_offset equal to length to make all data available immediately
        let mut reader = MockReader::with_alignment(mfp_data.clone(), mfp_data.len());

        // Load all data to simulate what the real reader would do
        reader.load_all_data().unwrap();

        // Test load with offset 0
        let size_result = MultiFragmentPacketRef::load(&mut reader, 0);
        assert!(size_result.is_ok());
        let size = size_result.unwrap();

        // Size should be aligned to 4 bytes
        let expected_size = (mfp_data.len() + 3) & !3; // Align up to 4 bytes
        assert_eq!(size, expected_size);
    }

    #[test]
    fn test_load_with_offset() {
        let mut data = vec![0; 8]; // Padding before MFP
        let mfp_data = create_test_mfp();
        data.extend_from_slice(&mfp_data);

        let mut reader = MockReader::with_alignment(data.clone(), data.len());

        // Load all data to simulate what the real reader would do
        reader.load_all_data().unwrap();

        // Test load with offset 8
        let size_result = MultiFragmentPacketRef::load(&mut reader, 8);
        assert!(size_result.is_ok());

        let size = size_result.unwrap();
        // Size should be aligned to 4 bytes
        let expected_size = (mfp_data.len() + 3) & !3; // Align up to 4 bytes
        assert_eq!(size, expected_size);
    }

    #[test]
    fn test_load_not_enough_data() {
        // Create an MFP
        let mfp_data = create_test_mfp();

        // Set write_offset to only half the data to simulate partial data availability
        let mut reader = MockReader::with_alignment(mfp_data.clone(), mfp_data.len() / 2);

        // Load all data available (which is only half)
        reader.load_all_data().unwrap();

        // This should fail because there's not enough data
        let result = MultiFragmentPacketRef::load(&mut reader, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_cast_success() {
        let mfp_data = create_test_mfp();

        // Cast should succeed with valid data
        let result = MultiFragmentPacketRef::cast(&mfp_data);
        assert!(result.is_ok());

        let mfp = result.unwrap();
        assert_eq!(mfp.magic(), 0x40CE);
        assert_eq!(mfp.event_id(), 1);
        assert_eq!(mfp.source_id(), 1);
        assert_eq!(mfp.fragment_count(), 2);
    }

    #[test]
    fn test_cast_corrupted_magic() {
        let mut mfp_data = create_test_mfp();

        // Corrupt the magic bytes
        mfp_data[0] = 0xFF;
        mfp_data[1] = 0xFF;

        // Cast should fail with corrupted magic
        let result = MultiFragmentPacketRef::cast(&mfp_data);
        assert!(result.is_err());

        match result {
            Err(err) => {
                let error_message = format!("{}", err);
                assert!(error_message.contains("Expected magic bytes"));
            },
            _ => panic!("Expected error but got success"),
        }
    }

    #[test]
    fn test_cast_not_enough_data() {
        let mfp_data = create_test_mfp();

        // Only use the first few bytes
        let partial_data = &mfp_data[0..10];

        // Cast should fail with not enough data
        let result = MultiFragmentPacketRef::cast(partial_data);
        assert!(result.is_err());

        match result {
            Err(err) => {
                let error_message = format!("{}", err);
                assert!(error_message.contains("Not enough data available"));
            },
            _ => panic!("Expected error but got success"),
        }
    }

    #[test]
    fn test_read_integration() {
        let mfp_data = create_test_mfp();
        let mut reader = MockReader::with_alignment(mfp_data.clone(), mfp_data.len());

        // Load all data to simulate what the real reader would do
        reader.load_all_data().unwrap();

        // Use the read method from ZeroCopyRingBufferReadable trait
        let result = MultiFragmentPacketRef::read(&mut reader);
        assert!(result.is_ok());

        let guard = result.unwrap();
        let mfp = guard.data_ref();

        // Verify the data is correct
        assert_eq!(mfp.magic(), 0x40CE);
        assert_eq!(mfp.event_id(), 1);
        assert_eq!(mfp.source_id(), 1);
        assert_eq!(mfp.fragment_count(), 2);

        // Check that we can access the fragments
        assert_eq!(mfp.fragment_data(0), Some(&[0, 1, 2, 3][..]));
        assert_eq!(mfp.fragment_data(1), Some(&[4, 5, 6, 7, 8][..]));
    }

    #[test]
    fn test_read_multiple_integration() {
        // Create two MFPs back-to-back
        let mfp1 = create_test_mfp();
        let mfp2 = create_test_mfp(); // Same content but separate instance

        let mut combined_data = Vec::new();
        combined_data.extend_from_slice(&mfp1);
        combined_data.extend_from_slice(&mfp2);

        let mut reader = MockReader::with_alignment(combined_data.clone(), combined_data.len());

        // Load all data to simulate what the real reader would do
        reader.load_all_data().unwrap();

        // Use the read_multiple method
        let result = MultiFragmentPacketRef::read_multiple(&mut reader, 2);
        assert!(result.is_ok());

        let guard = result.unwrap();

        // Check that we can access both MFPs
        assert_eq!(guard.data_ref(0).magic(), 0x40CE);
        assert_eq!(guard.data_ref(0).fragment_count(), 2);

        assert_eq!(guard.data_ref(1).magic(), 0x40CE);
        assert_eq!(guard.data_ref(1).fragment_count(), 2);
    }

    #[test]
    fn test_incremental_loading() {
        let mfp_data = create_test_mfp();

        // Initially set the write offset to just cover the header
        let mut reader = MockReader::with_alignment(mfp_data.clone(), 20);

        // Load data up to the current write offset
        reader.load_all_data().unwrap();

        // This should fail because we need the full packet
        let result = MultiFragmentPacketRef::load(&mut reader, 0);
        assert!(result.is_err());

        // Now simulate more data arriving - increase the write offset
        // We can't directly change the write offset, so we'll discard what we've seen
        // and create a new reader with more data available
        let mut reader = MockReader::with_alignment(mfp_data.clone(), mfp_data.len());

        // Load data up to the new write offset
        reader.load_all_data().unwrap();

        // Now we should be able to load the full packet
        let result = MultiFragmentPacketRef::load(&mut reader, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_discarding_data() {
        let mfp_data = create_test_mfp();
        let packet_size = mfp_data.len();

        let mut reader = MockReader::with_alignment(mfp_data.clone(), packet_size);
        reader.load_all_data().unwrap();

        // Read the packet
        let guard = MultiFragmentPacketRef::read(&mut reader).unwrap();

        // Verify data is accessible
        let mfp = guard.data_ref();
        assert_eq!(mfp.magic(), 0x40CE);

        // Discard the packet
        guard.discard().unwrap();

        // After discarding, the reader's read pointer should have advanced
        let data = reader.data();
        assert_eq!(data.len(), 0); // All data has been discarded
    }
}
 */

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

#[cfg(test)]
mod tests {
    use crate::mock_reader::MockReader;
    use crate::multi_fragment_packet::{
        Fragment, MultiFragmentPacketBuilder, MultiFragmentPacketRef,
    };
    use crate::typed_zero_copy_ring_buffer_reader::{CastBytesRef, ZeroCopyRingBufferReadable};
    use crate::utils;
    use crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

    // Helper function to create a test MFP with predictable content
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

    // Create a test MFP with proper alignment padding at the end
    fn create_aligned_test_mfp(align_power: u8) -> Vec<u8> {
        let mfp = create_test_mfp();
        let alignment = 1 << align_power;
        let aligned_size = utils::align_up(mfp.len(), alignment);
        let mut aligned_data = mfp;

        // Add padding to reach alignment boundary
        aligned_data.resize(aligned_size, 0);
        aligned_data
    }

    #[test]
    fn test_load_success() {
        let mfp_data = create_test_mfp();
        // Use alignment = None for this basic test
        let mut reader = MockReader::new(mfp_data.clone(), mfp_data.len());

        // Load all data to simulate what the real reader would do
        reader.load_all_data().unwrap();

        // Test load with offset 0
        let size_result = MultiFragmentPacketRef::load(&mut reader, 0);
        assert!(size_result.is_ok());
        let size = size_result.unwrap();

        // Size should be exactly the packet size (no alignment)
        assert_eq!(size, mfp_data.len());
    }

    #[test]
    fn test_load_with_offset() {
        let mut data = vec![0; 8]; // Padding before MFP
        let mfp_data = create_test_mfp();
        data.extend_from_slice(&mfp_data);

        let mut reader = MockReader::new(data.clone(), data.len());

        // Load all data to simulate what the real reader would do
        reader.load_all_data().unwrap();

        // Test load with offset 8
        let size_result = MultiFragmentPacketRef::load(&mut reader, 8);
        assert!(size_result.is_ok());

        let size = size_result.unwrap();
        // Size should be exactly the packet size (no alignment)
        assert_eq!(size, mfp_data.len());
    }

    #[test]
    fn test_load_not_enough_data() {
        let mfp_data = create_test_mfp();
        let mut reader = MockReader::new(mfp_data.clone(), mfp_data.len() / 2);
        reader.load_all_data().unwrap();

        // Verify it fails with NotEnoughDataAvailable error
        use crate::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadableError;
        match MultiFragmentPacketRef::load(&mut reader, 0) {
            Err(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable { .. }) => (),
            Err(err) => panic!("Wrong error variant: {:?}", err),
            Ok(_) => panic!("Expected error but got success"),
        }
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
        mfp_data[0] = 0xFF;
        mfp_data[1] = 0xFF;

        // Verify it fails with ImproperlyFormattedData error
        use crate::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadableError;
        match MultiFragmentPacketRef::cast(&mfp_data) {
            Err(ZeroCopyRingBufferReadableError::ImproperlyFormattedData { .. }) => (),
            Err(err) => panic!("Wrong error variant: {:?}", err),
            Ok(_) => panic!("Expected error but got success"),
        }
    }

    #[test]
    fn test_cast_not_enough_data() {
        let mfp_data = create_test_mfp();
        let partial_data = &mfp_data[0..10];

        // Verify it fails with NotEnoughDataAvailable error
        use crate::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadableError;
        match MultiFragmentPacketRef::cast(partial_data) {
            Err(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable { .. }) => (),
            Err(err) => panic!("Wrong error variant: {:?}", err),
            Ok(_) => panic!("Expected error but got success"),
        }
    }

    #[test]
    fn test_read_integration() {
        let mfp_data = create_test_mfp();
        let mut reader = MockReader::new(mfp_data.clone(), mfp_data.len());

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
        // Create two MFPs back-to-back with proper alignment
        let align_power = 2; // 2^2 = 4 byte alignment
        let mfp1 = create_aligned_test_mfp(align_power);
        let mfp2 = create_aligned_test_mfp(align_power);

        let mut combined_data = Vec::new();
        combined_data.extend_from_slice(&mfp1);
        combined_data.extend_from_slice(&mfp2);

        // Use alignment in the reader to match the data alignment
        let mut reader = MockReader::with_alignment(
            combined_data.clone(),
            combined_data.len(),
            1 << align_power,
        );

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
    fn test_multiple_read_and_discard() {
        // Create three MFPs back-to-back with proper alignment
        let align_power = 2; // 2^2 = 4 byte alignment
        let mfp1 = create_aligned_test_mfp(align_power);
        let mfp2 = create_aligned_test_mfp(align_power);
        let mfp3 = create_aligned_test_mfp(align_power);

        let mut combined_data = Vec::new();
        combined_data.extend_from_slice(&mfp1);
        combined_data.extend_from_slice(&mfp2);
        combined_data.extend_from_slice(&mfp3);

        // Use alignment in the reader to match data alignment
        let mut reader = MockReader::with_alignment(combined_data.clone(), combined_data.len(), 1 << align_power);
        reader.load_all_data().unwrap();

        // Read first two MFPs
        let guard1 = MultiFragmentPacketRef::read_multiple(&mut reader, 2).unwrap();

        // Check both MFPs
        assert_eq!(guard1.data_ref(0).magic(), 0x40CE);
        assert_eq!(guard1.data_ref(1).magic(), 0x40CE);

        // Discard first two packets
        guard1.discard().unwrap();

        // Should now be able to read the third packet
        let guard2 = MultiFragmentPacketRef::read(&mut reader).unwrap();
        assert_eq!(guard2.data_ref().magic(), 0x40CE);

        // Discard the third packet
        guard2.discard().unwrap();

        // All data should be consumed
        assert_eq!(reader.data().len(), 0);
    }

    #[test]
    fn test_alignment_different_sizes() {
        // Test with different alignment values
        for align_power in 1..4 {
            let alignment = 1 << align_power;

            // Create a packet
            let mfp_data = create_test_mfp();

            // Calculate what the aligned size should be
            let expected_aligned_size = utils::align_up(mfp_data.len(), alignment);

            // Create a reader with this alignment
            let mut reader = MockReader::with_alignment(mfp_data.clone(), mfp_data.len(), alignment);
            reader.load_all_data().unwrap();

            // Load the packet
            let size = MultiFragmentPacketRef::load(&mut reader, 0).unwrap();

            // Size should match our calculation
            assert_eq!(size, expected_aligned_size,
                       "Alignment mismatch with 2^{} alignment", align_power);
        }
    }
}

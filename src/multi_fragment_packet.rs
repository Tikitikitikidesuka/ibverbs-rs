use crate::typed_zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReadable, ZeroCopyRingBufferReadableError, ensure_available_bytes,
};
use crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

const HEADER_SIZE: usize = size_of::<MultiFragmentPacketRef<'_>>();
const MAGIC_BYTES: u16 = 0x40CE;

#[repr(C, packed)]
pub struct MultiFragmentPacketRef<'a> {
    magic: u16,
    fragment_count: u16,
    packet_size: u16,
    event_id: u32,
    source_id: u16,
    align: u8,
    fragment_version: u8,
    // Array of fragment types is dynamically sized [FragmentType]
    // Array of fragment sizes is dynamically sized [FragmentSize]
    // Array of fragments is dynamically sized [Fragment ([u8])]
    _phantom: std::marker::PhantomData<&'a [u8]>,
}

type FragmentType = u8;
type FragmentSize = u16;

pub enum MultiFragmentPacketFromRawBytesError {
    NotEnoughData {
        available_bytes: usize,
        required_bytes: usize,
    },
    CorruptedMagic {
        read_magic: u16,
        expected_magic: u16,
    },
}

impl<'a> MultiFragmentPacketRef<'a> {
    pub fn from_raw_bytes(data: &[u8]) -> Result<&Self, MultiFragmentPacketFromRawBytesError> {
        // Check if there is enough data for the header
        if data.len() < HEADER_SIZE {
            Err(MultiFragmentPacketFromRawBytesError::NotEnoughData {
                required_bytes: HEADER_SIZE,
                available_bytes: data.len(),
            })?;
        }

        // Cast to MFPRef type to read its attributes
        let mfp = unsafe { &*(data.as_ptr() as *const MultiFragmentPacketRef) };

        // Check the magic bytes are not corrupt
        if mfp.magic() != MAGIC_BYTES {
            Err(MultiFragmentPacketFromRawBytesError::CorruptedMagic {
                read_magic: mfp.magic(),
                expected_magic: MAGIC_BYTES,
            })?
        }

        // Check if there is enough data for the whole packet
        let packet_size = mfp.packet_size() as usize;
        if data.len() < packet_size {
            Err(MultiFragmentPacketFromRawBytesError::NotEnoughData {
                required_bytes: HEADER_SIZE,
                available_bytes: packet_size,
            })?;
        }

        Ok(mfp)
    }

    pub fn magic(&self) -> u16 {
        self.magic
    }

    pub fn fragment_count(&self) -> u16 {
        self.fragment_count
    }

    pub fn packet_size(&self) -> u16 {
        self.packet_size
    }

    pub fn event_id(&self) -> u32 {
        self.event_id
    }

    pub fn source_id(&self) -> u16 {
        self.source_id
    }

    pub fn align(&self) -> u8 {
        self.align
    }

    pub fn fragment_version(&self) -> u8 {
        self.fragment_version
    }
}

impl<'buf, R> ZeroCopyRingBufferReadable<'buf, R> for MultiFragmentPacketRef<'buf>
where
    R: ZeroCopyRingBufferReader,
{
    fn load(reader: &mut R, offset: usize) -> Result<usize, ZeroCopyRingBufferReadableError> {
        // Ensure enough data for the header
        ensure_available_bytes(reader, offset + HEADER_SIZE)?;

        // Get temporary access to the data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[offset..(offset + HEADER_SIZE)];
        let mfp = unsafe { &*(header_data.as_ptr() as *const MultiFragmentPacketRef<'_>) };

        // Get the total packet size from the header
        let packet_size = mfp.packet_size() as usize;

        // Ensure enough data for the whole mfp
        ensure_available_bytes(reader, offset + packet_size)?;

        Ok(packet_size)
    }

    fn cast(data: &[u8]) -> Result<&Self, ZeroCopyRingBufferReadableError> {
        MultiFragmentPacketRef::from_raw_bytes(data).map_err(|error| match error {
            MultiFragmentPacketFromRawBytesError::NotEnoughData {
                required_bytes,
                available_bytes,
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

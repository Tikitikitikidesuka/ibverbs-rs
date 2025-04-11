use crate::typed_zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReadable, ZeroCopyRingBufferReadableError, ensure_available_bytes,
};
use crate::utils;
use crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;
use std::fmt::{Debug, Display};
use std::slice;
use thiserror::Error;

const HEADER_SIZE: usize = size_of::<MultiFragmentPacketRef<'_>>();
const MAGIC_BYTES: u16 = 0x40CE;

#[repr(C, packed)]
pub struct MultiFragmentPacketRef<'a> {
    magic: u16,
    fragment_count: u16,
    packet_size: u32,
    event_id: u64,
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

#[derive(PartialEq, Eq)]
pub struct Fragment<'a> {
    fragment_type: FragmentType,
    fragment_size: FragmentSize,
    data: &'a [u8],
}

#[derive(Debug, Error)]
pub enum MultiFragmentPacketFromRawBytesError {
    #[error(
        "Not enough data available: Required {required_data} bytes. Only {available_data} bytes are available in the buffer"
    )]
    NotEnoughDataAvailable {
        available_data: usize,
        required_data: usize,
    },

    #[error(
        "Magic bytes on the header are corrupted: Expected {expected_magic:x?}, found {read_magic:x?}"
    )]
    CorruptedMagic {
        read_magic: u16,
        expected_magic: u16,
    },
}

impl<'a> MultiFragmentPacketRef<'a> {
    pub fn from_raw_bytes(data: &[u8]) -> Result<&Self, MultiFragmentPacketFromRawBytesError> {
        // Check if there is enough data for the header
        if data.len() < HEADER_SIZE {
            Err(
                MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                    required_data: HEADER_SIZE,
                    available_data: data.len(),
                },
            )?;
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
            Err(
                MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                    required_data: HEADER_SIZE,
                    available_data: packet_size,
                },
            )?;
        }

        Ok(mfp)
    }

    pub fn magic(&self) -> u16 {
        self.magic
    }

    pub fn fragment_count(&self) -> u16 {
        self.fragment_count
    }

    pub fn packet_size(&self) -> u32 {
        self.packet_size
    }

    pub fn event_id(&self) -> u64 {
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

    pub fn fragment_type(&self, index: usize) -> Option<FragmentType> {
        if index < self.fragment_count() as usize {
            let fragment_type_ptr = unsafe { self.fragment_type_ptr().add(index) };
            let fragment_type = unsafe { *fragment_type_ptr };
            Some(fragment_type)
        } else {
            None
        }
    }

    pub fn fragment_size(&self, index: usize) -> Option<FragmentSize> {
        if index < self.fragment_count() as usize {
            let fragment_size_ptr = unsafe { self.fragment_size_ptr().add(index) };
            let fragment_size = unsafe { *fragment_size_ptr };
            Some(fragment_size)
        } else {
            None
        }
    }

    /// No random access, O(n)
    pub fn fragment_data(&self, index: usize) -> Option<&[u8]> {
        Some(self.iter().nth(index)?.data)
    }

    /// No random access, O(n)
    pub fn fragment(&self, index: usize) -> Option<Fragment> {
        self.iter().nth(index)
    }

    pub fn iter(&self) -> MultiFragmentPacketRefIter {
        MultiFragmentPacketRefIter {
            packet: self,
            offset: 0,
            index: 0,
        }
    }

    pub fn raw_packet_data(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                (self as *const Self) as *const u8,
                self.packet_size as usize,
            )
        }
    }

    unsafe fn fragment_type_ptr(&self) -> *const FragmentType {
        unsafe { ((self as *const Self) as *const u8).add(HEADER_SIZE) as *const FragmentType }
    }

    unsafe fn fragment_size_ptr(&self) -> *const FragmentSize {
        let fragment_types_size = self.fragment_count() as usize * size_of::<FragmentType>();
        let aligned_fragment_types_size = utils::align_up_2pow(fragment_types_size, 2); // 32 bit alignment -> 4 bytes -> 2^2
        unsafe {
            (self.fragment_type_ptr() as *const u8).add(aligned_fragment_types_size)
                as *const FragmentSize
        }
    }

    unsafe fn fragment_data_ptr(&self) -> *const u8 {
        let fragment_sizes_size = self.fragment_count() as usize * size_of::<FragmentSize>();
        let aligned_fragment_sizes_size = utils::align_up_2pow(fragment_sizes_size, 2); // 32 bit alignment -> 4 bytes -> 2^2
        unsafe { (self.fragment_size_ptr() as *const u8).add(aligned_fragment_sizes_size) }
    }
}

struct MultiFragmentPacketRefIter<'a> {
    packet: &'a MultiFragmentPacketRef<'a>,
    offset: usize,
    index: usize,
}

impl<'a> Iterator for MultiFragmentPacketRefIter<'a> {
    type Item = Fragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let fragment_type = self.packet.fragment_type(self.index)?;
        let fragment_size = self.packet.fragment_size(self.index)?;

        let data_start = unsafe { self.packet.fragment_data_ptr().add(self.offset) };
        let data = unsafe { slice::from_raw_parts(data_start, fragment_size as usize) };

        self.offset += utils::align_up_2pow(fragment_size as usize, self.packet.align());
        self.index += 1;

        Some(Fragment {
            fragment_type,
            fragment_size,
            data,
        })
    }
}

impl<'a> ExactSizeIterator for MultiFragmentPacketRefIter<'a> {
    fn len(&self) -> usize {
        self.packet.fragment_count() as usize
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

    fn cast(data: &[u8]) -> Result<&Self, ZeroCopyRingBufferReadableError> {
        MultiFragmentPacketRef::from_raw_bytes(data).map_err(|error| match error {
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

impl<'a> Debug for MultiFragmentPacketRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiFragmentPacketRef")
            .field("magic", &format!("0x{:04X}", self.magic()))
            .field("fragment_count", &self.fragment_count())
            .field("packet_size", &self.packet_size())
            .field("event_id", &self.event_id())
            .field("source_id", &self.source_id())
            .field("align", &self.align())
            .field("fragment_version", &self.fragment_version())
            .finish()
    }
}

impl<'a> Display for MultiFragmentPacketRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MFP[magic=0x{:04X}, fragments={}, packet_size={}, event_id={}, source_id={}, fragment_version={}]",
            self.magic(),
            self.fragment_count(),
            self.packet_size(),
            self.event_id(),
            self.source_id(),
            self.fragment_version()
        )
    }
}

#[derive(Debug)]
struct FragmentDebugView {
    fragment_type: FragmentType,
    fragment_size: FragmentSize,
    data_preview: String,
}

impl<'a> Debug for Fragment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data_preview = if self.data.len() > 16 {
            format!("{:02X?}... ({} bytes)", &self.data[0..16], self.data.len())
        } else {
            format!("{:02X?}", self.data)
        };

        f.debug_struct("Fragment")
            .field("type", &self.fragment_type)
            .field("size", &self.fragment_size)
            .field("data", &data_preview)
            .finish()
    }
}

impl<'a> Display for Fragment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Fragment[type={}, size={}]",
            self.fragment_type, self.fragment_size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_multi_fragment_packet_data() -> Vec<u8> {
        [
            vec![0xCE, 0x40],                           // Magic (0xCE40)
            vec![5, 0],                                 // Fragment count (5)
            vec![64, 0, 0, 0],                          // Packet size (64)
            vec![1, 0, 0, 0, 0, 0, 0, 0],               //Event id (1)
            vec![1, 0],                                 // Source id (1)
            vec![3],                                    // Align (2^3)
            vec![1],                                    // Fragment version (1)
            vec![0, 1, 2, 3, 4],                        // Fragment types [0, 1, 2, 3, 4]
            vec![0, 0, 0],                              // Padding to 32 bits
            vec![4, 0, 5, 0, 8, 0, 9, 0, 12, 0],        // Fragment sizes [4, 5, 8, 9, 12]
            vec![0, 0],                                 // Padding to 32 bits
            vec![0, 1, 2, 3],                           // Fragment 0
            vec![0, 0, 0, 0],                           // Padding to 2^3
            vec![0, 1, 2, 3, 4],                        // Fragment 1
            vec![0, 0, 0],                              // Padding to 2^3
            vec![0, 1, 2, 3, 4, 5, 6, 7],               // Fragment 2
            vec![],                                     // Padding to 2^3
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8],            // Fragment 3
            vec![0, 0, 0, 0, 0, 0, 0],                  // Padding to 2^3
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], // Fragment 4
            vec![0, 0, 0, 0],                           // Padding to 2^3
        ]
        .concat()
    }

    #[test]
    fn test_mfp_magic_packet_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.magic(), 0x40CE);
    }

    #[test]
    fn test_mfp_fragment_count_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.fragment_count(), 5);
    }

    #[test]
    fn test_mfp_packet_size_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.packet_size(), 64);
    }

    #[test]
    fn test_mfp_event_id_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.event_id(), 1);
    }

    #[test]
    fn test_mfp_source_id_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.source_id(), 1);
    }

    #[test]
    fn test_mfp_align_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.align(), 3);
    }

    #[test]
    fn test_mfp_fragment_version_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.fragment_version(), 1);
    }

    #[test]
    fn test_mfp_fragment_type_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();

        // Check all fragment types
        assert_eq!(mfp.fragment_type(0), Some(0));
        assert_eq!(mfp.fragment_type(1), Some(1));
        assert_eq!(mfp.fragment_type(2), Some(2));
        assert_eq!(mfp.fragment_type(3), Some(3));
        assert_eq!(mfp.fragment_type(4), Some(4));

        // Check out of bounds
        assert_eq!(mfp.fragment_type(5), None);
    }

    #[test]
    fn test_mfp_fragment_size_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();

        // Check all fragment sizes
        assert_eq!(mfp.fragment_size(0), Some(4));
        assert_eq!(mfp.fragment_size(1), Some(5));
        assert_eq!(mfp.fragment_size(2), Some(8));
        assert_eq!(mfp.fragment_size(3), Some(9));
        assert_eq!(mfp.fragment_size(4), Some(12));

        // Check out of bounds
        assert_eq!(mfp.fragment_size(5), None);
    }

    #[test]
    fn test_mfp_fragment_data_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();

        // Check all fragment data
        assert_eq!(mfp.fragment_data(0), Some(&[0, 1, 2, 3][..]));
        assert_eq!(mfp.fragment_data(1), Some(&[0, 1, 2, 3, 4][..]));
        assert_eq!(mfp.fragment_data(2), Some(&[0, 1, 2, 3, 4, 5, 6, 7][..]));
        assert_eq!(mfp.fragment_data(3), Some(&[0, 1, 2, 3, 4, 5, 6, 7, 8][..]));
        assert_eq!(
            mfp.fragment_data(4),
            Some(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..])
        );

        // Check out of bounds
        assert_eq!(mfp.fragment_data(5), None);
    }

    #[test]
    fn test_mfp_fragment_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();

        // Check first fragment using direct comparison
        let expected_fragment0 = Fragment {
            fragment_type: 0,
            fragment_size: 4,
            data: &[0, 1, 2, 3][..],
        };
        assert_eq!(mfp.fragment(0).unwrap(), expected_fragment0);

        // Check last fragment using direct comparison
        let expected_fragment4 = Fragment {
            fragment_type: 4,
            fragment_size: 12,
            data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
        };
        assert_eq!(mfp.fragment(4).unwrap(), expected_fragment4);

        // Check out of bounds
        assert_eq!(mfp.fragment(5), None);
    }

    #[test]
    fn test_mfp_iter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();

        let expected_fragments = vec![
            Fragment {
                fragment_type: 0,
                fragment_size: 4,
                data: &[0, 1, 2, 3][..],
            },
            Fragment {
                fragment_type: 1,
                fragment_size: 5,
                data: &[0, 1, 2, 3, 4][..],
            },
            Fragment {
                fragment_type: 2,
                fragment_size: 8,
                data: &[0, 1, 2, 3, 4, 5, 6, 7][..],
            },
            Fragment {
                fragment_type: 3,
                fragment_size: 9,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8][..],
            },
            Fragment {
                fragment_type: 4,
                fragment_size: 12,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
            },
        ];

        let fragments: Vec<Fragment> = mfp.iter().collect();
        assert_eq!(fragments, expected_fragments);
    }

    #[test]
    fn test_exact_size_iterator() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();

        let iter = mfp.iter();
        assert_eq!(iter.len(), 5);

        // After consuming some elements, len() should still report total length
        let mut iter = mfp.iter();
        iter.next();
        iter.next();
        assert_eq!(iter.len(), 5);

        // Confirm we can iterate through all elements
        let mut count = 0;
        let mut iter = mfp.iter();
        while let Some(_) = iter.next() {
            count += 1;
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn test_mfp_raw_packet_data() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::from_raw_bytes(&data).unwrap();

        let raw_data = mfp.raw_packet_data();

        // The raw packet data should be the same as the input data up to packet_size
        assert_eq!(raw_data.len(), 64);
        assert_eq!(raw_data, &data[0..64]);
    }
}

pub mod builder;

#[cfg(feature = "pcie40-io")]
pub mod pcie40_readable;

#[cfg(feature = "shmem-io")]
pub mod shared_memory_element;

pub use builder::MultiFragmentPacketBuilder;
pub mod fragment_type;
pub mod odin;
pub mod sub_detector;

use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::mem::offset_of;
use std::ops::Deref;
use std::slice;
use thiserror::Error;

use crate::fragment_type::FragmentType;

impl MultiFragmentPacketRef {
    pub const VALID_MAGIC: u16 = 0x40CE;
    pub const HEADER_SIZE: usize = size_of::<MultiFragmentPacketHeader>();
}

/// Type of a source id.
pub type SourceId = u16;
pub type EventId = u64;

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultiFragmentPacketHeader {
    magic: u16,
    fragment_count: u16,
    packet_size: u32,
    event_id: EventId,
    source_id: SourceId,
    align: u8,
    fragment_version: u8,
}

pub struct MultiFragmentPacket {
    data: Vec<u8>,
    // Array of fragment types is dynamically sized [FragmentType]
    // Array of fragment sizes is dynamically sized [FragmentSize]
    // Array of fragments is dynamically sized [Fragment ([u8])]
}

impl MultiFragmentPacket {
    /// # Safety
    /// Vec needs to contain a valid [`MultiFragmentPacket`].
    #[must_use]
    pub unsafe fn from_data(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn builder() -> MultiFragmentPacketBuilder {
        MultiFragmentPacketBuilder::default()
    }
}

#[repr(C, packed)]
pub struct MultiFragmentPacketRef {
    header: MultiFragmentPacketHeader,
    // Array of fragment types is dynamically sized [FragmentType]
    // Array of fragment sizes is dynamically sized [FragmentSize]
    // Array of fragments is dynamically sized [Fragment ([u8])]
}

impl AsRef<MultiFragmentPacketRef> for MultiFragmentPacket {
    fn as_ref(&self) -> &MultiFragmentPacketRef {
        // MultiFragmentPacket must be guaranteed to be correct already. Since it can only
        // be built by the builder, it is supposed to be guaranteed.
        unsafe { MultiFragmentPacketRef::unchecked_ref_from_raw_bytes(self.data.as_slice()) }
    }
}

impl Deref for MultiFragmentPacket {
    type Target = MultiFragmentPacketRef;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl ToOwned for MultiFragmentPacketRef {
    type Owned = MultiFragmentPacket;

    fn to_owned(&self) -> Self::Owned {
        Self::Owned {
            data: self.raw_packet_data().to_vec(),
        }
    }
}

impl Borrow<MultiFragmentPacketRef> for MultiFragmentPacket {
    fn borrow(&self) -> &MultiFragmentPacketRef {
        self
    }
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Fragment<'a, Data: ?Sized = [u8]> {
    r#type: u8,
    version: u8,
    event_id: EventId,
    source_id: SourceId,
    data: &'a Data,
}

impl<'a, T: ?Sized> Fragment<'a, T> {
    pub fn new(
        r#type: u8,
        version: u8,
        event_id: EventId,
        source_id: SourceId,
        data: &'a T,
    ) -> Self {
        Fragment {
            r#type,
            version,
            event_id,
            source_id,
            data,
        }
    }

    pub fn fragment_type_raw(&self) -> u8 {
        self.r#type
    }

    pub fn fragment_type_parsed(&self) -> Option<FragmentType> {
        FragmentType::from_repr(self.fragment_type_raw())
    }

    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub fn event_id(&self) -> EventId {
        self.event_id
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn payload(&self) -> &T {
        self.data
    }

    /// in bytes, excluding the header
    #[must_use]
    pub fn fragment_size(&self) -> u16 {
        size_of_val(self.data)
            .try_into()
            .expect("fragment size fits u16")
    }
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

impl MultiFragmentPacketRef {
    pub fn ref_from_raw_bytes(data: &[u8]) -> Result<&Self, MultiFragmentPacketFromRawBytesError> {
        // Check if there is enough data for the header
        if data.len() < Self::HEADER_SIZE {
            Err(
                MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                    required_data: Self::HEADER_SIZE,
                    available_data: data.len(),
                },
            )?;
        }

        let mfp = unsafe { Self::unchecked_ref_from_raw_bytes(data) };

        // Check the magic bytes are not corrupt
        if mfp.magic() != Self::VALID_MAGIC {
            Err(MultiFragmentPacketFromRawBytesError::CorruptedMagic {
                read_magic: mfp.magic(),
                expected_magic: Self::VALID_MAGIC,
            })?
        }

        Ok(mfp)
    }

    pub fn magic_field_offset() -> usize {
        offset_of!(MultiFragmentPacketHeader, magic)
    }

    pub fn magic_field_size() -> usize {
        size_of::<u16>()
    }

    pub fn magic(&self) -> u16 {
        unsafe { self.header().magic }
    }

    pub fn fragment_count(&self) -> u16 {
        unsafe { self.header().fragment_count }
    }

    /// Packet size **in byets** including header.
    pub fn packet_size(&self) -> u32 {
        unsafe { self.header().packet_size }
    }

    /// Event ID of first fragment in this packet.
    pub fn event_id(&self) -> EventId {
        unsafe { self.header().event_id }
    }

    /// Fragments in this packet are padded to 2^`align` bytes.
    pub fn source_id(&self) -> SourceId {
        unsafe { self.header().source_id }
    }

    /// Fragments in this packet are padded to 2^`align` bytes.
    pub fn align(&self) -> u8 {
        unsafe { self.header().align }
    }

    /// Version of the data format of the fragments.
    pub fn fragment_version(&self) -> u8 {
        unsafe { self.header().fragment_version }
    }

    pub fn fragment_type(&self, index: usize) -> Option<u8> {
        if index < self.fragment_count() as usize {
            let fragment_type_ptr = unsafe { self.fragment_type_ptr().add(index) };
            let fragment_type = unsafe { *fragment_type_ptr };
            Some(fragment_type)
        } else {
            None
        }
    }

    /// in bytes, excluding the header (only data)
    pub fn fragment_size(&self, index: usize) -> Option<u16> {
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
    pub fn fragment(&self, index: usize) -> Option<Fragment<'_>> {
        self.iter().nth(index)
    }

    pub fn iter(&self) -> MultiFragmentPacketIter<'_> {
        MultiFragmentPacketIter {
            packet: self,
            offset: 0,
            index: 0,
        }
    }

    pub fn raw_packet_data(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                (self as *const Self) as *const u8,
                self.header().packet_size as usize,
            )
        }
    }

    unsafe fn unchecked_ref_from_raw_bytes(data: &[u8]) -> &Self {
        // Cast to MFPRef type to read its attributes
        unsafe { &*(data.as_ptr() as *const MultiFragmentPacketRef) }
    }

    unsafe fn header(&self) -> &MultiFragmentPacketHeader {
        unsafe { &*(self as *const Self as *const MultiFragmentPacketHeader) }
    }

    unsafe fn fragment_type_ptr(&self) -> *const u8 {
        unsafe { ((self as *const Self) as *const u8).add(Self::HEADER_SIZE) }
    }

    unsafe fn fragment_size_ptr(&self) -> *const u16 {
        let fragment_types_size = self.fragment_count() as usize * size_of::<u8>();
        let aligned_fragment_types_size = alignment_utils::align_up_pow2(fragment_types_size, 2); // 32 bit alignment -> 4 bytes -> 2^2
        unsafe { self.fragment_type_ptr().add(aligned_fragment_types_size) as *const u16 }
    }

    unsafe fn fragment_data_ptr(&self) -> *const u8 {
        let fragment_sizes_size = self.fragment_count() as usize * size_of::<u16>();
        let aligned_fragment_sizes_size = alignment_utils::align_up_pow2(fragment_sizes_size, 2); // 32 bit alignment -> 4 bytes -> 2^2
        unsafe { (self.fragment_size_ptr() as *const u8).add(aligned_fragment_sizes_size) }
    }
}

pub struct MultiFragmentPacketIter<'a> {
    packet: &'a MultiFragmentPacketRef,
    offset: usize,
    index: usize,
}

impl<'a> Iterator for MultiFragmentPacketIter<'a> {
    type Item = Fragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let fragment_type = self.packet.fragment_type(self.index)?;
        let fragment_size = self.packet.fragment_size(self.index)?;

        let data_start = unsafe { self.packet.fragment_data_ptr().add(self.offset) };
        let data = unsafe { slice::from_raw_parts(data_start, fragment_size as usize) };

        let event_id = self.packet.event_id() + self.index as EventId;

        self.offset += alignment_utils::align_up_pow2(fragment_size as usize, self.packet.align());
        self.index += 1;

        Some(Fragment {
            event_id,
            source_id: self.packet.source_id(),
            version: self.packet.fragment_version(),
            r#type: fragment_type,
            data,
        })
    }
}

impl ExactSizeIterator for MultiFragmentPacketIter<'_> {
    fn len(&self) -> usize {
        self.packet.fragment_count() as usize
    }
}

impl Debug for MultiFragmentPacketRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiFragmentPacketRef")
            .field("magic", &format!("{:#04X}", self.magic()))
            .field("fragment_count", &self.fragment_count())
            .field("packet_size", &self.packet_size())
            .field("event_id", &self.event_id())
            .field("source_id", &self.source_id())
            .field("align", &self.align())
            .field("fragment_version", &self.fragment_version())
            .finish()
    }
}

impl Display for MultiFragmentPacketRef {
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

impl Debug for Fragment<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data_preview = if self.data.len() > 16 {
            format!("{:02X?}... ({} bytes)", &self.data[0..16], self.data.len())
        } else {
            format!("{:02X?}", self.data)
        };

        f.debug_struct("Fragment")
            .field("type", &self.r#type)
            .field("size", &self.fragment_size())
            .field("data", &data_preview)
            .field("version", &self.version)
            .field("event_id", &self.event_id)
            .field("source_id", &self.source_id)
            .finish()
    }
}

impl Display for Fragment<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Fragment[type={}, size={}]",
            self.r#type,
            self.fragment_size()
        )
    }
}

#[cfg(feature = "bincode")]
mod bincode {
    use super::*;
    use ::bincode;
    use bincode::{de::read::Reader, enc::write::Writer};
    impl bincode::Decode<()> for MultiFragmentPacket {
        fn decode<D: bincode::de::Decoder<Context = ()>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            const HEADER_SIZE: usize = size_of::<MultiFragmentPacketHeader>();

            let mut bytes: [u8; HEADER_SIZE] = Default::default();
            decoder.reader().read(&mut bytes)?;

            let header = unsafe { &*(bytes.as_ptr() as *const MultiFragmentPacketHeader) };

            if header.magic != MultiFragmentPacketRef::VALID_MAGIC {
                let magic = header.magic;
                return Err(bincode::error::DecodeError::OtherString(format!(
                    "Invalid magic number for `MultiEventPacket`: got {magic:#04X} but expected {:#04X}",
                    MultiFragmentPacketRef::VALID_MAGIC
                )));
            }

            let mut data = vec![0u8; header.packet_size as usize];
            data[0..HEADER_SIZE].copy_from_slice(&bytes);
            decoder.reader().read(&mut data[HEADER_SIZE..])?;

            Ok(Self { data })
        }
    }

    impl bincode::Encode for MultiFragmentPacket {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            self.as_ref().encode(encoder)
        }
    }

    impl bincode::Encode for MultiFragmentPacketRef {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            encoder.writer().write(self.raw_packet_data())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_multi_fragment_packet_data() -> Vec<u8> {
        [
            vec![0xCE, 0x40],                           // Magic (0xCE40)
            vec![5, 0],                                 // Fragment count (5)
            vec![96, 0, 0, 0],                          // Packet size (96)
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
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.magic(), 0x40CE);
    }

    #[test]
    fn test_mfp_fragment_count_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.fragment_count(), 5);
    }

    #[test]
    fn test_mfp_packet_size_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.raw_packet_data().len(), mfp.packet_size() as usize);
        assert_eq!(mfp.packet_size(), 96);
    }

    #[test]
    fn test_mfp_event_id_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.event_id(), 1);
    }

    #[test]
    fn test_mfp_source_id_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.source_id(), 1);
    }

    #[test]
    fn test_mfp_align_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.align(), 3);
    }

    #[test]
    fn test_mfp_fragment_version_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.fragment_version(), 1);
    }

    #[test]
    fn test_mfp_fragment_type_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();

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
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();

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
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();

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
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();

        // Check first fragment using direct comparison
        let expected_fragment0 = Fragment {
            r#type: 0,
            version: 1,
            event_id: 1,
            source_id: 1,
            data: &[0, 1, 2, 3][..],
        };
        assert_eq!(mfp.fragment(0).unwrap(), expected_fragment0);

        // Check last fragment using direct comparison
        let expected_fragment4 = Fragment {
            r#type: 4,
            source_id: 1,
            event_id: 5,
            version: 1,
            data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
        };
        assert_eq!(mfp.fragment(4).unwrap(), expected_fragment4);

        // Check out of bounds
        assert_eq!(mfp.fragment(5), None);
    }

    #[test]
    fn test_mfp_iter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();

        let expected_fragments = vec![
            Fragment {
                r#type: 0,
                data: &[0, 1, 2, 3][..],
                version: 1,
                event_id: 1,
                source_id: 1,
            },
            Fragment {
                r#type: 1,
                data: &[0, 1, 2, 3, 4][..],
                version: 1,
                event_id: 2,
                source_id: 1,
            },
            Fragment {
                r#type: 2,
                data: &[0, 1, 2, 3, 4, 5, 6, 7][..],
                version: 1,
                event_id: 3,
                source_id: 1,
            },
            Fragment {
                r#type: 3,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8][..],
                version: 1,
                event_id: 4,
                source_id: 1,
            },
            Fragment {
                r#type: 4,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
                version: 1,
                event_id: 5,
                source_id: 1,
            },
        ];

        let fragments: Vec<Fragment> = mfp.iter().collect();
        assert_eq!(fragments, expected_fragments);
    }

    #[test]
    fn test_exact_size_iterator() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();

        let iter = mfp.iter();
        assert_eq!(iter.len(), 5);

        // After consuming some elements, len() should still report total length
        let mut iter = mfp.iter();
        iter.next();
        iter.next();
        assert_eq!(iter.len(), 5);

        // Confirm we can iterate through all elements
        let mut count = 0;
        let iter = mfp.iter();
        for _ in iter {
            count += 1;
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn test_mfp_raw_packet_data() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacketRef::ref_from_raw_bytes(&data).unwrap();

        let raw_data = mfp.raw_packet_data();

        // The raw packet data should be the same as the input data up to packet_size
        assert_eq!(raw_data.len(), data.len());
        assert_eq!(raw_data, &data);
    }
}

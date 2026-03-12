#![doc = include_str!("../README.md")]
pub mod builder;

#[cfg(feature = "pcie40-io")]
pub mod pcie40_readable;

#[cfg(feature = "shmem-io")]
pub mod shared_memory_element;

pub mod odin_mock;

pub use builder::MultiFragmentPacketBuilder;
use bytemuck::{Pod, Zeroable};
use ebutils::fragment::Fragment;
use ebutils::source_id::SourceId;
use ebutils::{END_OF_RUN, EventId, Uninstantiatable};
pub mod owned;

pub use owned::MultiFragmentPacketOwned;

use std::fmt::{Debug, Display};
use std::mem::offset_of;
use std::ops::Range;
use std::slice;
use thiserror::Error;

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

#[repr(C, packed)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct MultiFragmentPacketHeader {
    magic: u16,
    fragment_count: u16,
    packet_size: u32,
    event_id: EventId,
    source_id: SourceId,
    align: u8,
    fragment_version: u8,
}

/// This struct represents a multi-fragment packet in memory.
///
/// It can be thought of as similar to [`str`] in a way that it only ever exist behind references `&MultiFragmentPacket`, never owned.
/// If you want an owned version, use [`MultiFragmentPacketOwned`].
/// There also exists a builder for that using [`MultiFragmentPacketOwned::builder`].
/// Its relationship to [`MultiFragmentPacketOwned`] is as [`str`] to [`String`].
///
/// See the [module level documentation](crate#what-is-an-mfp) for more details on what an MFP actually represents.
/// The MFP format is defined [here](https://edms.cern.ch/ui/file/2100937/5/edms_2100937_raw_data_format_run3.pdf#section.3).
// todo add an external type once they stabilize github.com/rust-lang/rust/issues/43467
#[repr(C, packed)]
pub struct MultiFragmentPacket {
    header: MultiFragmentPacketHeader,
    // Array of fragment types is dynamically sized [FragmentType]
    // Array of fragment sizes is dynamically sized [FragmentSize]
    // Array of fragments is dynamically sized [Fragment ([u8])]
    _unin: Uninstantiatable,
}

impl MultiFragmentPacket {
    /// The valid value for the header magic field.
    pub const VALID_MAGIC: u16 = 0x40CE;
    /// The size of the header in bytes.
    pub const HEADER_SIZE: usize = size_of::<MultiFragmentPacketHeader>();

    /// Casts a byte slice to an MFP checking for the magic number and size.
    ///
    /// The passed slice must only be at least as large as the MFP, it may be larger as well.
    pub fn from_raw_bytes(data: &[u8]) -> Result<&Self, FromRawBytesError> {
        // Check if there is enough data for the header
        if data.len() < Self::HEADER_SIZE {
            Err(FromRawBytesError::NotEnoughDataAvailable {
                required_data: Self::HEADER_SIZE,
                available_data: data.len(),
            })?;
        }

        let header: &MultiFragmentPacketHeader = bytemuck::from_bytes(&data[..Self::HEADER_SIZE]);

        // Check the magic bytes are not corrupt
        if header.magic != Self::VALID_MAGIC {
            Err(FromRawBytesError::CorruptedMagic {
                read_magic: header.magic,
                expected_magic: Self::VALID_MAGIC,
            })?
        }

        if header.packet_size as usize > data.len() {
            Err(FromRawBytesError::NotEnoughDataAvailable {
                required_data: header.packet_size as usize,
                available_data: data.len(),
            })?;
        }

        //  SAFETY: data slice is large enough
        let mfp = unsafe { Self::unchecked_ref_from_raw_bytes(data) };
        Ok(mfp)
    }

    #[allow(unused)]
    pub(crate) fn magic_field_offset() -> usize {
        offset_of!(MultiFragmentPacketHeader, magic)
    }

    #[allow(unused)]
    pub(crate) fn magic_field_size() -> usize {
        size_of::<u16>()
    }

    /// Returns the magic number stored in the header.
    pub fn magic(&self) -> u16 {
        self.header().magic
    }

    /// Returns the number of fragments in this packet.
    pub fn fragment_count(&self) -> u16 {
        self.header().fragment_count
    }

    /// Returns the packet size **in byets** including header.
    pub fn packet_size(&self) -> u32 {
        self.header().packet_size
    }

    /// Returns the Event ID of first fragment in this packet.
    ///
    /// The event ids of the fragments are sequential, so the event id of fragment `n` is `event_id() + n`.
    pub fn event_id(&self) -> EventId {
        self.header().event_id
    }

    /// Returns true if thes MFP marks the end of a run, i.e. has event id [`END_OF_RUN`].
    ///
    /// Those MFPs may contain fragments, but they are empty.
    pub fn is_end_of_run(&self) -> bool {
        self.event_id() == END_OF_RUN
    }

    /// Returns the Source ID of all of the fragments in this packet.
    ///
    /// One MFP always originates from a single source.
    pub fn source_id(&self) -> SourceId {
        self.header().source_id
    }

    /// Fragments in this packet are padded to 2^`align_log` bytes.
    pub fn align_log(&self) -> u8 {
        self.header().align
    }

    /// Returns the version of the data format of the fragments.
    ///
    /// Each fragment in an MFP has the same version.
    pub fn fragment_version(&self) -> u8 {
        self.header().fragment_version
    }

    /// Returns the type of the fragment at the given index.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn fragment_type(&self, index: usize) -> Option<u8> {
        if index < self.fragment_count() as usize {
            let fragment_type_ptr = unsafe { self.fragment_type_ptr().add(index) };
            let fragment_type = unsafe { *fragment_type_ptr };
            Some(fragment_type)
        } else {
            None
        }
    }

    /// Returns the size in bytes, excluding the header (only data) of the fragment at the given index.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn fragment_size(&self, index: usize) -> Option<u16> {
        if index < self.fragment_count() as usize {
            let fragment_size_ptr = unsafe { self.fragment_size_ptr().add(index) };
            let fragment_size = unsafe { *fragment_size_ptr };
            Some(fragment_size)
        } else {
            None
        }
    }

    /// Returns the data of the fragment at the given index as a byte slice.
    ///
    /// For more convenient access, consider using [`Self::fragment`] instead.
    ///
    /// No random access, `O(n)`. Use [`Self::fragment_iter`] instead when accessing multiple/all fragments.
    pub fn fragment_data(&self, index: usize) -> Option<&[u8]> {
        let frag = self.fragment_iter().nth(index)?;
        Some(frag.payload_bytes())
    }

    /// Returns a reference to the fragment at the given index.
    ///
    /// No random access, `O(n)`. Use [`Self::fragment_iter`] instead when accessing multiple/all fragments.
    pub fn fragment(&self, index: usize) -> Option<Fragment<'_>> {
        self.fragment_iter().nth(index)
    }

    /// Returns an iterator over all fragments in this packet.
    ///
    /// The iterator yields [`Fragment`] references, which can tried to be cast to a specific fragment type, e.g. with [`Fragment::try_into_odin`].
    pub fn fragment_iter(&self) -> MultiFragmentPacketIter<'_> {
        MultiFragmentPacketIter {
            packet: self,
            offset: 0,
            index: 0,
        }
    }

    /// Returns the entire packed as byte slice.
    pub fn raw_packet_data(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                (self as *const Self) as *const u8,
                self.header().packet_size as usize,
            )
        }
    }

    /// Returns a range over the event ids this MFP contains.
    pub fn event_id_range(&self) -> Range<EventId> {
        let start = self.event_id();

        Range {
            start,
            end: start + self.fragment_count() as EventId,
        }
    }

    /// Reinterprets a byte slice as a reference to a MultiFragmentPacket without any checks.
    /// # Safety
    /// The passed data must be at least as large as the header size, and as the size indicated in the header.
    unsafe fn unchecked_ref_from_raw_bytes(data: &[u8]) -> &Self {
        // SAFETY: See function preconditions
        unsafe { &*(data.as_ptr().cast()) }
    }

    fn header(&self) -> &MultiFragmentPacketHeader {
        &self.header
    }

    // todo reduce the unsafe code hereafter.
    unsafe fn fragment_type_ptr(&self) -> *const u8 {
        unsafe { ((self as *const Self) as *const u8).add(Self::HEADER_SIZE) }
    }

    unsafe fn fragment_size_ptr(&self) -> *const u16 {
        let fragment_types_size = self.fragment_count() as usize * size_of::<u8>();
        let aligned_fragment_types_size = ebutils::align_up_pow2(fragment_types_size, 2); // 32 bit alignment -> 4 bytes -> 2^2
        unsafe { self.fragment_type_ptr().add(aligned_fragment_types_size) as *const u16 }
    }

    unsafe fn fragment_data_ptr(&self) -> *const u8 {
        let fragment_sizes_size = self.fragment_count() as usize * size_of::<u16>();
        let aligned_fragment_sizes_size = ebutils::align_up_pow2(fragment_sizes_size, 2); // 32 bit alignment -> 4 bytes -> 2^2
        unsafe { (self.fragment_size_ptr() as *const u8).add(aligned_fragment_sizes_size) }
    }
}

/// Errors that can be encountered when trying to construct a MultiFragmentPacket from raw bytes.
#[derive(Debug, Error)]
pub enum FromRawBytesError {
    /// Not enough bytes presented to decode MFP.
    #[error(
        "Not enough data available: Required {required_data} bytes. Only {available_data} bytes are available in the buffer"
    )]
    NotEnoughDataAvailable {
        available_data: usize,
        required_data: usize,
    },

    /// Invalid magic for an MFP.
    #[error(
        "Magic bytes on the header are corrupted: Expected {expected_magic:x?}, found {read_magic:x?}"
    )]
    CorruptedMagic {
        read_magic: u16,
        expected_magic: u16,
    },
}

/// An iterator for iterating over all the [`Fragment`]s in a [`MultiFragmentPacket`].
pub struct MultiFragmentPacketIter<'a> {
    packet: &'a MultiFragmentPacket,
    offset: usize,
    index: usize,
}

impl<'a> Iterator for MultiFragmentPacketIter<'a> {
    type Item = Fragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let fragment_type = self.packet.fragment_type(self.index)?;
        let fragment_size = self.packet.fragment_size(self.index)?;

        // todo this may be possible without unsafe code using a body slice
        let data_start = unsafe { self.packet.fragment_data_ptr().add(self.offset) };
        let data = unsafe { slice::from_raw_parts(data_start, fragment_size as usize) };

        let event_id = self.packet.event_id() + self.index as EventId;

        self.offset += ebutils::align_up_pow2(fragment_size as usize, self.packet.align_log());
        self.index += 1;

        Some(Fragment::new(
            fragment_type,
            self.packet.fragment_version(),
            event_id,
            self.packet.source_id(),
            data,
        ))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.packet.fragment_count() as usize - self.index;
        (size, Some(size))
    }
}

impl ExactSizeIterator for MultiFragmentPacketIter<'_> {} // size shall be implemented in Iterator

impl Debug for MultiFragmentPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let frags = self
            .fragment_iter()
            .map(|f| match f.try_into_odin() {
                Ok(odin) => Box::new(odin) as Box<dyn Debug>,
                Err(_) => Box::new(f) as Box<dyn Debug>,
            })
            .collect::<Vec<_>>();
        f.debug_struct("MultiFragmentPacket")
            .field("magic", &format!("{:#04X}", self.magic()))
            .field("fragment_count", &self.fragment_count())
            .field("packet_size", &self.packet_size())
            .field("event_id", &self.event_id())
            .field("source_id", &self.source_id())
            .field("align", &self.align_log())
            .field("fragment_version", &self.fragment_version())
            .field("fragments", &frags)
            .finish()
    }
}

impl Display for MultiFragmentPacket {
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
#[cfg(feature = "bincode")]
mod bincode {
    use super::*;
    use ::bincode;
    use bincode::{de::read::Reader, enc::write::Writer};
    impl bincode::Decode<()> for MultiFragmentPacketOwned {
        fn decode<D: bincode::de::Decoder<Context = ()>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            const HEADER_SIZE: usize = size_of::<MultiFragmentPacketHeader>();

            let mut bytes: [u8; HEADER_SIZE] = Default::default();
            decoder.reader().read(&mut bytes)?;

            let header = unsafe { &*(bytes.as_ptr() as *const MultiFragmentPacketHeader) };

            if header.magic != MultiFragmentPacket::VALID_MAGIC {
                let magic = header.magic;
                return Err(bincode::error::DecodeError::OtherString(format!(
                    "Invalid magic number for `MultiEventPacket`: got {magic:#04X} but expected {:#04X}",
                    MultiFragmentPacket::VALID_MAGIC
                )));
            }

            let mut data = vec![0u8; header.packet_size as usize];
            data[0..HEADER_SIZE].copy_from_slice(&bytes);
            decoder.reader().read(&mut data[HEADER_SIZE..])?;

            // SAFETY: is a valid MFP in terms of the function because magic and size match.
            Ok(unsafe { Self::from_data_unchecked(data) })
        }
    }

    impl bincode::Encode for MultiFragmentPacketOwned {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            self.as_ref().encode(encoder)
        }
    }

    impl bincode::Encode for MultiFragmentPacket {
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
    use ebutils::{fragment::Fragment, source_id::SourceId};

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
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.magic(), 0x40CE);
    }

    #[test]
    fn test_mfp_fragment_count_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.fragment_count(), 5);
    }

    #[test]
    fn test_mfp_packet_size_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.raw_packet_data().len(), mfp.packet_size() as usize);
        assert_eq!(mfp.packet_size(), 96);
    }

    #[test]
    fn test_mfp_event_id_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.event_id(), 1);
    }

    #[test]
    fn test_mfp_source_id_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.source_id().0, 1);
    }

    #[test]
    fn test_mfp_align_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.align_log(), 3);
    }

    #[test]
    fn test_mfp_fragment_version_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();
        assert_eq!(mfp.fragment_version(), 1);
    }

    #[test]
    fn test_mfp_fragment_type_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();

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
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();

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
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();

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
    fn test_exact_size_iterator() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();

        let iter = mfp.fragment_iter();
        assert_eq!(iter.len(), 5);

        let mut iter = mfp.fragment_iter();
        iter.next();
        iter.next();
        assert_eq!(iter.len(), 3);

        // Confirm we can iterate through all elements
        let mut count = 0;
        let iter = mfp.fragment_iter();
        for _ in iter {
            count += 1;
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn test_mfp_raw_packet_data() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();

        let raw_data = mfp.raw_packet_data();

        // The raw packet data should be the same as the input data up to packet_size
        assert_eq!(raw_data.len(), data.len());
        assert_eq!(raw_data, &data);
    }

    #[test]
    fn test_mfp_fragment_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();

        // Check first fragment using direct comparison
        let expected_fragment0 = Fragment::new(0, 1, 1, SourceId(1), &[0, 1, 2, 3][..]);
        assert_eq!(mfp.fragment(0).unwrap(), expected_fragment0);

        // Check last fragment using direct comparison
        let expected_fragment4 = Fragment::new(
            4,
            1,
            5,
            SourceId(1),
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
        );
        assert_eq!(mfp.fragment(4).unwrap(), expected_fragment4);

        // Check out of bounds
        assert_eq!(mfp.fragment(5), None);
    }

    #[test]
    fn test_mfp_iter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::from_raw_bytes(&data).unwrap();

        let expected_fragments = vec![
            Fragment::new(0, 1, 1, SourceId(1), &[0, 1, 2, 3][..]),
            Fragment::new(1, 1, 2, SourceId(1), &[0, 1, 2, 3, 4][..]),
            Fragment::new(2, 1, 3, SourceId(1), &[0, 1, 2, 3, 4, 5, 6, 7][..]),
            Fragment::new(3, 1, 4, SourceId(1), &[0, 1, 2, 3, 4, 5, 6, 7, 8][..]),
            Fragment::new(
                4,
                1,
                5,
                SourceId(1),
                &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
            ),
        ];

        let fragments: Vec<Fragment> = mfp.fragment_iter().collect();
        assert_eq!(fragments, expected_fragments);
    }
}

use std::{
    borrow::Borrow,
    fmt::Debug,
    ops::{Deref, Range},
    slice,
};

use multi_fragment_packet::{MultiFragmentPacketRef, SourceId};

use crate::builder::MultiEventPacketBuilder;

pub mod builder;

pub struct MultiEventPacket {
    data: Box<[u32]>, // assures alignement of u32
}

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

#[derive(Copy, Clone)]
#[repr(C, packed(4))] // alignment of u32 ensured
/// Just the constant-sized part of the MEP header.
pub(crate) struct MultiEventPacketConstHeader {
    magic: u16,
    num_mfps: u16,
    /// Packet size in 32 bit words
    packet_size: u32,
}

impl MultiEventPacketRef {
    pub const MAGIC: u16 = 0xCEFA;
}

impl Deref for MultiEventPacket {
    type Target = MultiEventPacketRef;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsRef<MultiEventPacketRef> for MultiEventPacket {
    fn as_ref(&self) -> &MultiEventPacketRef {
        // MultiEventPacket must be guaranteed to be correct already. Since it can only
        // be built by the builder, it is supposed to be guaranteed.
        unsafe { MultiEventPacketRef::unchecked_ref_from_raw_bytes(&self.data) }
    }
}

impl Borrow<MultiEventPacketRef> for MultiEventPacket {
    fn borrow(&self) -> &MultiEventPacketRef {
        self
    }
}

impl MultiEventPacket {
    pub fn builder<'a>() -> MultiEventPacketBuilder<'a> {
        MultiEventPacketBuilder::new()
    }
}

#[repr(C, packed)]
pub struct MultiEventPacketRef {
    header: MultiEventPacketConstHeader,
}

impl ToOwned for MultiEventPacketRef {
    type Owned = MultiEventPacket;

    fn to_owned(&self) -> Self::Owned {
        Self::Owned {
            data: self.data_aligned().to_vec().into_boxed_slice(),
        }
    }
}

impl MultiEventPacketRef {
    pub fn magic(&self) -> u16 {
        self.header.magic
    }

    pub fn num_mfps(&self) -> u16 {
        self.header.num_mfps
    }

    // Size of packet **in 32 bit words!** (as stored in the header).
    pub fn packet_size_u32(&self) -> u32 {
        self.header.packet_size
    }

    pub fn packet_size_byets(&self) -> usize {
        self.packet_size_u32() as usize * size_of::<u32>()
    }

    pub fn mfp_source_ids(&self) -> &[SourceId] {
        // SAFETY: Source ids start ofter constant sized header part.
        let src_ids = unsafe {
            (self as *const Self as *const SourceId)
                .byte_add(size_of::<MultiEventPacketConstHeader>())
        };
        // SAFETY: Source ids have have 16 bit size and are located here sequentially, alignment is correct.
        unsafe { slice::from_raw_parts(src_ids, self.num_mfps() as _) }
    }

    /// Offset of the mfps from the start of the header, **in 32 bit words!** (as stored in the header).
    pub fn mfp_offsets_u32(&self) -> &[Offset] {
        // SAFETY: Offsets are located after the constant header part and source ids, padded to an even number (32 bit).
        let offsets = unsafe {
            (self as *const Self as *const u32)
                .byte_add(size_of::<MultiEventPacketConstHeader>())
                .byte_add(self.src_ids_size())
        };

        // SAFETY: Offsets have 32 bit size and are located here sequentially, alignment is correct.
        unsafe { slice::from_raw_parts(offsets, self.num_mfps() as _) }
    }

    pub fn mfp_offset_bytes(&self, idx: usize) -> Option<usize> {
        self.mfp_offsets_u32()
            .get(idx)
            .map(|v| *v as usize * size_of::<u32>())
    }

    pub fn get_mep(&self, idx: usize) -> Option<&MultiFragmentPacketRef> {
        // SAFETY: MFPs are located at the given offset (in bytes!) and expected to be valid.
        // SAFETY: Returned lifetime is same as data
        self.mfp_offset_bytes(idx).map(|off| unsafe {
            &*(self as *const Self as *const MultiFragmentPacketRef).byte_add(off)
        })
    }

    pub fn header_size(&self) -> usize {
        header_size(self.num_mfps() as _)
    }

    fn src_ids_size(&self) -> usize {
        src_ids_size(self.num_mfps() as _)
    }

    pub fn mfp_iter(&self) -> MultiEventPacketIterator<'_> {
        MultiEventPacketIterator {
            mep: self,
            next_idx: 0,
            end: None,
        }
    }

    pub fn mfp_iter_srcid_range(&self, range: Range<SourceId>) -> MultiEventPacketIterator<'_> {
        let start_idx = self.mfp_source_ids().partition_point(|v| *v < range.start);
        let end_idx = self.mfp_source_ids().partition_point(|v| *v < range.end);

        MultiEventPacketIterator {
            mep: self,
            next_idx: start_idx,
            end: Some(end_idx),
        }
    }

    pub fn data(&self) -> &[u8] {
        // SAFETY: Data of length packet_size (in bytes!) belongs to this MEP. Returned lifetime is same as of self.
        unsafe { slice::from_raw_parts(self as *const Self as *const u8, self.packet_size_byets()) }
    }

    pub fn data_aligned(&self) -> &[u32] {
        // SAFETY: Data of length packet_size (in u32!) belongs to this MEP. Returned lifetime is same as of self.
        // SAFETY: Alignment is guaranteed to be of of u32.
        unsafe {
            slice::from_raw_parts(
                self as *const Self as *const u32,
                self.packet_size_u32() as _,
            )
        }
    }

    /// SAFETY: Assumes data contains a valid MEP, with MFPs sorted by srcid.
    unsafe fn unchecked_ref_from_raw_bytes(data: &[u32]) -> &Self {
        // SAFETY: Data contains valid MEP and returned lifetime is same as of data.
        unsafe { &*(data.as_ptr() as *const MultiEventPacketRef) }
    }
}

impl Debug for MultiEventPacketRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mfps = self.mfp_iter().collect::<Vec<_>>();
        f.debug_struct("MultiEventPacketRef")
            .field("magic", &format!("{:#04X}", self.magic()))
            .field("nmfps", &self.num_mfps())
            .field("pwords", &self.packet_size_u32())
            .field("srcids", &self.mfp_source_ids())
            .field("offsets", &self.mfp_offsets_u32())
            .field("mfps", &mfps.as_slice())
            .finish()
    }
}

impl Debug for MultiEventPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}

/// Type of MFP offsets as in the MEP header.
pub type Offset = u32;

pub(crate) fn src_ids_size(num_mfps: usize) -> usize {
    const MFP_ROUNDING: usize = 2;
    num_mfps.next_multiple_of(MFP_ROUNDING) * size_of::<SourceId>()
}

pub(crate) fn offsets_size(num_mfps: usize) -> usize {
    num_mfps * size_of::<u32>()
}

pub(crate) fn header_size(num_mfps: usize) -> usize {
    size_of::<MultiEventPacketConstHeader>() + src_ids_size(num_mfps) + offsets_size(num_mfps)
}

pub struct MultiEventPacketIterator<'a> {
    mep: &'a MultiEventPacketRef,
    next_idx: usize,
    end: Option<usize>,
}

impl<'a> Iterator for MultiEventPacketIterator<'a> {
    type Item = &'a MultiFragmentPacketRef;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end.is_none_or(|end| self.next_idx < end) {
            let ret = self.mep.get_mep(self.next_idx);
            self.next_idx += 1;
            ret
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remain = self.end.unwrap_or(self.mep.num_mfps() as usize) - self.next_idx;
        (remain, Some(remain))
    }
}

impl<'a> ExactSizeIterator for MultiEventPacketIterator<'a> {}

pub(crate) fn slice_as_bytes_mut(data: &mut [u32]) -> &mut [u8] {
    // SAFETY: slice is contigous without padding of correct length. Lifetime matches.
    unsafe { slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u8, std::mem::size_of_val(data)) }
}

#[cfg(feature = "bincode")]
mod bincode {
    use ::bincode;
    use bincode::{Decode, Encode, de::read::Reader, enc::write::Writer};

    use crate::{MultiEventPacket, MultiEventPacketConstHeader, MultiEventPacketRef};

    impl Decode<()> for MultiEventPacket {
        fn decode<D: bincode::de::Decoder<Context = ()>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            const HEADER_SIZE: usize = size_of::<MultiEventPacketConstHeader>() / size_of::<u32>();
            union Header {
                typed: MultiEventPacketConstHeader,
                bytes: [u32; HEADER_SIZE],
            }

            let mut bytes: [u32; _] = Default::default();
            decoder
                .reader()
                .read(super::slice_as_bytes_mut(&mut bytes))?;
            let header = Header { bytes };

            // SAFETY: header has been received validly, packed size is size of entire packed in 32 bit words.
            let mut data = vec![0u32; unsafe { header.typed.packet_size } as usize];

            // SAFETY: repr(C) type can safely be accessed as bytes.
            data[0..HEADER_SIZE].copy_from_slice(unsafe { &header.bytes });
            decoder
                .reader()
                .read(super::slice_as_bytes_mut(&mut data[HEADER_SIZE..]))?;

            Ok(Self {
                data: data.into_boxed_slice(),
            })
        }
    }

    impl Encode for MultiEventPacketRef {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            encoder.writer().write(self.data())
        }
    }

    impl Encode for MultiEventPacket {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            self.as_ref().encode(encoder)
        }
    }
}

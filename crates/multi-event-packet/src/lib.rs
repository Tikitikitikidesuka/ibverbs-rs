use core::slice;
use std::ops::Deref;

use multi_fragment_packet::MultiFragmentPacketRef;

pub struct MultiEventPacket {
    data: Box<[u8]>,
}

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct MultiEventPacketConstHeader {
    magic: u16,
    num_mfps: u16,
    packet_size: u32,
}

impl MultiEventPacketRef {
    pub const MAGIC: u16 = 0xCEFA;
}

impl Deref for MultiEventPacket {
    type Target = MultiEventPacketRef;

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl AsRef<MultiEventPacketRef> for MultiEventPacket {
    fn as_ref(&self) -> &MultiEventPacketRef {
        // MultiEventPacket must be guaranteed to be correct already. Since it can only
        // be built by the builder, it is supposed to be guaranteed.
        unsafe { MultiEventPacketRef::unchecked_ref_from_raw_bytes(&self.data) }
    }
}

#[repr(C, packed)]
pub struct MultiEventPacketRef {
    header: MultiEventPacketConstHeader,
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

    const SOURCE_ID_SIZE: usize = size_of::<u16>();
    const OFFSET_SIZE: usize = size_of::<u32>();

    pub fn mfp_source_id(&self, idx: usize) -> Option<u16> {
        if idx > self.num_mfps() as usize {
            return None;
        }

        // SAFETY: Source ids start ofter constant sized header part.
        let src_ids = unsafe {
            (self as *const Self as *const u16).byte_add(size_of::<MultiEventPacketConstHeader>())
        };

        // SAFETY: Source ids have have 16 bit size and are located here, idx is not too large.
        let src_id = unsafe { *src_ids.add(idx) };

        Some(src_id)
    }

    /// Offset of the mfps from the start of the header, **in 32 bit words!** (as stored in the header).
    pub fn mfp_offset_u32(&self, idx: usize) -> Option<u32> {
        if idx > self.num_mfps() as usize {
            return None;
        }

        let source_ids_size = self.num_mfps().next_multiple_of(2) as usize * Self::SOURCE_ID_SIZE;

        // SAFETY: Offsets are located after the constant header part and source ids, padded to an even number (32 bit).
        let offsets = unsafe {
            (self as *const Self as *const u32)
                .byte_add(size_of::<MultiEventPacketConstHeader>())
                .byte_add(source_ids_size)
        };

        // SAFETY: Offsets have 32 bit size and are located here, idx is not too large.
        let offset = unsafe { *offsets.add(idx) };

        Some(offset)
    }

    pub fn mfp_offset_bytes(&self, idx: usize) -> Option<usize> {
        self.mfp_offset_u32(idx)
            .map(|v| v as usize * size_of::<u32>())
    }

    pub fn get_mep(&self, idx: usize) -> Option<&MultiFragmentPacketRef> {
        // SAFETY: MFPs are located at the given offset (in bytes!) and expected to be valid.
        self.mfp_offset_bytes(idx).map(|off| unsafe {
            &*(self as *const Self as *const MultiFragmentPacketRef).byte_add(off)
        })
    }

    pub fn header_size(&self) -> usize {
        size_of::<MultiEventPacketConstHeader>()
            + self.num_mfps().next_multiple_of(2) as usize * Self::SOURCE_ID_SIZE
            + self.num_mfps() as usize * Self::OFFSET_SIZE
    }

    pub fn mfp_iter(&self) -> MultiEventPacketIterator<'_> {
        MultiEventPacketIterator {
            mep: self,
            next_idx: 0,
        }
    }

    pub fn data(&self) -> &[u8] {
        // SAFETY: Data of length packet_size (in bytes!) belongs to this MEP.
        unsafe { slice::from_raw_parts(self as *const Self as _, self.packet_size_byets()) }
    }

    unsafe fn unchecked_ref_from_raw_bytes(data: &[u8]) -> &Self {
        // Cast to MEPRef type to read its attributes
        unsafe { &*(data.as_ptr() as *const MultiEventPacketRef) }
    }
}

pub struct MultiEventPacketIterator<'a> {
    mep: &'a MultiEventPacketRef,
    next_idx: usize,
}

impl<'a> Iterator for MultiEventPacketIterator<'a> {
    type Item = &'a MultiFragmentPacketRef;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.mep.get_mep(self.next_idx);
        self.next_idx += 1;
        ret
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.mep.num_mfps() as usize;
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for MultiEventPacketIterator<'a> {}

mod bincode {
    use ::bincode;
    use bincode::{Decode, Encode, de::read::Reader, enc::write::Writer};

    use crate::{MultiEventPacket, MultiEventPacketConstHeader, MultiEventPacketRef};

    impl Decode<()> for MultiEventPacket {
        fn decode<D: bincode::de::Decoder<Context = ()>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            const HEADER_SIZE: usize = size_of::<MultiEventPacketConstHeader>();
            union Header {
                typed: MultiEventPacketConstHeader,
                bytes: [u8; HEADER_SIZE],
            }

            let mut bytes: [u8; _] = Default::default();
            decoder.reader().read(&mut bytes)?;
            let header = Header { bytes };

            // SAFETY: header has been received validly, packed size is size of entire packed in 32 bit words.
            let mut data =
                vec![0u8; unsafe { header.typed.packet_size } as usize * size_of::<u32>()];

            // SAFETY: repr(C) type can safely be accessed as bytes.
            data[0..HEADER_SIZE].copy_from_slice(unsafe { &header.bytes });
            decoder.reader().read(&mut data[HEADER_SIZE..])?;

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
}

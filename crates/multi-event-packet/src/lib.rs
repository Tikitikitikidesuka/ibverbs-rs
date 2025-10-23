use std::ops::Deref;

use multi_fragment_packet::MultiFragmentPacketRef;

pub struct MultiEventPacket {
    data: Box<[u8]>,
}

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct MultiEventPacketConstHeader {
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

    const SOURCE_ID_SIZE: usize = size_of::<u16>();
    const OFFSET_SIZE: usize = size_of::<u32>();

    pub fn mfp_source_id(&self, idx: usize) -> Option<u16> {
        if idx > self.num_mfps() as usize {
            return None;
        }

        let src_ids = unsafe {
            (self as *const Self as *const u16).byte_add(size_of::<MultiEventPacketConstHeader>())
        };
        let src_id = unsafe { *src_ids.add(idx) };

        Some(src_id)
    }

    pub fn mfp_offset(&self, idx: usize) -> Option<u32> {
        if idx > self.num_mfps() as usize {
            return None;
        }

        let source_ids_size = self.num_mfps().next_multiple_of(2) as usize * Self::SOURCE_ID_SIZE;

        let offsets = unsafe {
            (self as *const Self as *const u32)
                .byte_add(size_of::<MultiEventPacketConstHeader>())
                .byte_add(source_ids_size)
        };

        let offset = unsafe { *offsets.add(idx) };

        Some(offset)
    }

    pub fn get_mep(&self, idx: usize) -> Option<&MultiFragmentPacketRef> {
        self.mfp_offset(idx).map(|off| unsafe {
            &*(self as *const Self as *const MultiFragmentPacketRef).byte_add(off as usize)
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

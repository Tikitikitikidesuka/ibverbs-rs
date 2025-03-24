use crate::mfp_ref::MFPRefError::CorruptData;

const MFP_MAGIC: u16 = 0x40CE;

pub enum MFPRefError {
    CorruptData(String),
}

#[repr(C, packed)]
pub struct MFPRefHeader {
    pub magic: u16,
    pub n_frags: u16,
    pub packet_size: u32,
    pub event_id: u32,
    pub src_id: u16,
    pub align: u8,
    pub block_version: u8,
}

type FragmentType = u8;
type FragmentSize = u16;

impl MFPRefHeader {
    pub fn magic(&self) -> u16 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.magic)) }
    }

    pub fn n_frags(&self) -> u16 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.n_frags)) }
    }

    pub fn packet_size(&self) -> u32 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.packet_size)) }
    }

    pub fn event_id(&self) -> u32 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.event_id)) }
    }

    pub fn src_id(&self) -> u16 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.src_id)) }
    }

    pub fn align(&self) -> u8 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.align)) }
    }

    pub fn block_version(&self) -> u8 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.block_version)) }
    }
}

impl<'a> MFPRefHeader {
    pub fn new(data: &'a [u8]) -> Result<&'a Self, MFPRefError> {
        // Check at least enough data for the header
        if data.len() < size_of::<MFPRefHeader>() {
            return Err(CorruptData("MFP data smaller than header size".to_string()));
        }

        // Get a reference to the header (Must make sure the data lives at least as much
        // as the reference and is not written into). This is ensured by the 'a lifetime.
        let header = unsafe { &*(data.as_ptr() as *const MFPRefHeader) };

        // Validate magic number
        let magic = header.magic();
        if magic != MFP_MAGIC {
            return Err(CorruptData(format!(
                "Invalid MFP magic: Got 0x{:04x}, expected {MFP_MAGIC}",
                magic
            )));
        }

        // Return correct header reference
        Ok(header)
    }
}

pub struct MFPRef<'a> {
    data: &'a [u8],
    header: &'a MFPRefHeader,
}

impl<'a> MFPRef<'a> {
    // Data must be of the whole MFP, not only of the data after the header.
    pub fn new(header: &'a MFPRefHeader, data: &'a [u8]) -> Result<Self, MFPRefError> {
        // Validate packet size
        let packet_size = header.packet_size();
        if packet_size as usize != data.len() {
            return Err(CorruptData(format!(
                "MFP size mismatch: header says {} but buffer has {}",
                packet_size,
                data.len()
            )));
        }

        // Return correct MFP reference
        Ok(MFPRef { data, header })
    }

    pub fn header(&self) -> &'a MFPRefHeader {
        self.header
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn is_end_run(&self) -> bool {
        todo!()
    }

    pub fn fragment_types(&self) -> &'a [FragmentType] {
        let n_frags = self.header.n_frags() as usize;
        let start = size_of::<MFPRefHeader>();
        let end = start + n_frags * size_of::<FragmentType>();
        &self.data[start..end]
    }

    pub fn fragment_sizes(&self) -> &'a [FragmentSize] {
        let n_frags = self.header.n_frags() as usize;
        let start =
            size_of::<MFPRefHeader>() + self.fragment_types().len() * size_of::<FragmentSize>();

        unsafe {
            std::slice::from_raw_parts(
                self.data.as_ptr().add(start) as *const FragmentSize,
                n_frags,
            )
        }
    }
}

use std::fmt::Display;
use crate::typed_zero_copy_ring_buffer_reader::{
    TypedDataGuard, TypedMultiDataGuard, ZeroCopyRingBufferReadable,
    ZeroCopyRingBufferReadableError, ensure_available_bytes,
};
use crate::zero_copy_ring_buffer_reader::{DataGuard, ZeroCopyRingBufferReader};
use std::mem::size_of;

/*
TestReadable structure:
- list_id: i32,
- element_count: i32,
- elements: [i32; element_count],
 */

#[derive(Debug)]
pub struct I32List<'a> {
    data: &'a [u8],
}

#[derive(Debug)]
#[repr(C, packed)]
struct I32ListHeader {
    list_id: i32,
    element_count: i32,
}

impl Display for I32List<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ Id: {}, Count: {}, Elements: {:?} ]", self.list_id(), self.element_count(), self.elements())
    }
}

impl<'a> I32List<'a> {
    pub fn from_data(data: &'a [u8]) -> Option<Self> {
        // Check if there's enough data for at least the header
        if data.len() < size_of::<I32ListHeader>() {
            return None;
        }

        // Read the header to check if there's enough data for all elements
        let header = unsafe { &*(data.as_ptr() as *const I32ListHeader) };
        let element_count = header.element_count as usize;
        let expected_size = size_of::<I32ListHeader>() + (element_count * size_of::<i32>());

        if data.len() < expected_size {
            return None;
        }

        // Return an owned I32List instead of a reference
        Some(I32List { data })
    }

    // Keep the rest of the methods unchanged
    pub fn list_id(&self) -> i32 {
        self.header().list_id
    }

    pub fn element_count(&self) -> i32 {
        self.header().element_count
    }

    pub fn elements(&self) -> &[i32] {
        let count = self.header().element_count as usize;
        let elements_offset = size_of::<I32ListHeader>();

        // Already validated the size in the constructor
        unsafe {
            std::slice::from_raw_parts(self.data.as_ptr().add(elements_offset) as *const i32, count)
        }
    }

    fn header(&self) -> &I32ListHeader {
        // Already checked there is enough data in the constructor
        I32ListHeader::new(&self.data[..size_of::<I32ListHeader>()]).unwrap()
    }
}

impl I32ListHeader {
    fn new(data: &[u8]) -> Option<&I32ListHeader> {
        if data.len() != size_of::<I32ListHeader>() {
            None
        } else {
            Some(unsafe { &*(data[..size_of::<I32ListHeader>()].as_ptr() as *const I32ListHeader) })
        }
    }
}

impl<'buf, R> ZeroCopyRingBufferReadable<'buf, R> for I32List<'buf>
where
    R: ZeroCopyRingBufferReader,
{
    fn load(
        reader: &mut R,
        offset: usize,
    ) -> Result<(DataGuard<R>, usize), ZeroCopyRingBufferReadableError> {
        // Check if there is enough data for the header
        const HEADER_SIZE: usize = size_of::<I32ListHeader>();

        // Ensure there are enough bytes available starting from the offset
        ensure_available_bytes(reader, offset + HEADER_SIZE)?;

        // Get temporary access to data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[offset..(offset + HEADER_SIZE)];
        let header = unsafe { &*(header_data.as_ptr() as *const I32ListHeader) };

        // Calculate the total size needed based on header information
        let element_count = header.element_count as usize;
        let total_size = HEADER_SIZE + element_count * size_of::<i32>();

        // Ensure enough data for the entire list starting from the offset
        ensure_available_bytes(reader, offset + total_size)?;

        // Get the final data guard with all required data
        Ok((reader.data(), total_size))
    }

    fn cast(data: &'buf [u8]) -> Result<Self, ZeroCopyRingBufferReadableError> {
        Self::from_data(data).ok_or(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
            required_data: size_of::<I32ListHeader>(),
            available_data: data.len(),
        })
    }
}

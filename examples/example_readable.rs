use pcie40_rs::typed_zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReadable, ZeroCopyRingBufferReadableError, ensure_available_bytes,
};
use pcie40_rs::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;
use std::fmt::{Debug, Display, Formatter};
use std::mem::size_of;

/*
I32List structure:
- list_id: i32,
- element_count: i32,
- elements: [i32; element_count],
 */

#[repr(C, packed)]
pub struct I32ListRef<'a> {
    list_id: i32,
    element_count: i32,
    // The elements field is not explicitly defined here
    // It will be accessed via pointer arithmetic
    _phantom: std::marker::PhantomData<&'a [u8]>,
}

impl<'a> I32ListRef<'a> {
    pub fn from_raw_bytes(data: &[u8]) -> Option<&Self> {
        // Check if there's enough data for at least the header
        if data.len() < size_of::<I32ListRef<'_>>() {
            return None;
        }

        // Read the header to check if there's enough data for all elements
        let list = unsafe { &*(data.as_ptr() as *const I32ListRef<'_>) };
        let element_count = list.element_count as usize;
        let expected_size = size_of::<I32ListRef<'_>>() + (element_count * size_of::<i32>());

        if data.len() < expected_size {
            return None;
        }

        // Return a reference to the I32List
        Some(list)
    }

    pub fn list_id(&self) -> i32 {
        self.list_id
    }

    pub fn element_count(&self) -> i32 {
        self.element_count
    }

    pub fn elements(&self) -> &'a [i32] {
        let count = self.element_count as usize;

        // Use pointer arithmetic to access the elements that follow the struct
        unsafe {
            let elements_ptr = (self as *const Self).add(1) as *const i32;
            std::slice::from_raw_parts(elements_ptr, count)
        }
    }
}

impl<'buf, R> ZeroCopyRingBufferReadable<'buf, R> for I32ListRef<'buf>
where
    R: ZeroCopyRingBufferReader,
{
    fn load(reader: &mut R, offset: usize) -> Result<usize, ZeroCopyRingBufferReadableError> {
        // Check if there is enough data for at least the header
        const HEADER_SIZE: usize = size_of::<I32ListRef<'_>>();
        ensure_available_bytes(reader, offset + HEADER_SIZE)?;

        // Get temporary access to data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[offset..(offset + HEADER_SIZE)];
        let list = unsafe { &*(header_data.as_ptr() as *const I32ListRef<'_>) };

        // Calculate the total size needed based on header information
        let element_count = list.element_count as usize;
        let total_size = HEADER_SIZE + (element_count * size_of::<i32>());

        // Ensure enough data for the entire list starting from the offset
        ensure_available_bytes(reader, offset + total_size)?;

        // Get the final data guard with all required data
        Ok(total_size)
    }

    fn cast(data: &[u8]) -> Result<&Self, ZeroCopyRingBufferReadableError> {
        I32ListRef::from_raw_bytes(data).ok_or_else(|| {
            ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                required_data: size_of::<I32ListRef<'_>>(),
                available_data: data.len(),
            }
        })
    }
}

impl Debug for I32ListRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[Id: {}, Count: {}, Elements: {:?}]",
            self.list_id(),
            self.element_count(),
            self.elements()
        )
    }
}

impl Display for I32ListRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "I32List(id={}, count={}, elements=[{}])",
            self.list_id(),
            self.element_count(),
            self.elements()
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn main() {}

use crate::typed_zero_copy_ring_buffer_reader::{
    TypedDataGuard, TypedMultiDataGuard, ZeroCopyRingBufferReadable,
    ZeroCopyRingBufferReadableError,
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

impl<'a> I32List<'a> {
    // Change to return an owned I32List instead of a reference
    pub fn new(data: &'a [u8]) -> Option<Self> {
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
        reader: &'buf mut R,
        offset: usize,
    ) -> Result<(DataGuard<'buf, R>, usize), ZeroCopyRingBufferReadableError> {
        // Check if there is enough data for the header
        const HEADER_SIZE: usize = size_of::<I32ListHeader>();
        let available_data = reader.data().len();

        if available_data < HEADER_SIZE {
            // Try to load more data
            let loaded_data = reader
                .load_data(HEADER_SIZE - available_data)
                .map_err(|error| {
                    ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
                })?;

            // Check if we have enough data now
            if available_data + loaded_data < HEADER_SIZE {
                return Err(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                    required_data: HEADER_SIZE,
                    available_data: available_data + loaded_data,
                });
            }
        }

        // Get temporary access to data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[..HEADER_SIZE];
        let header = unsafe { &*(header_data.as_ptr() as *const I32ListHeader) };

        // Calculate the total size needed based on header information
        let element_count = header.element_count as usize;
        let total_size = HEADER_SIZE + element_count * size_of::<i32>();

        // Drop the temporary data access before potentially loading more
        //drop(temp_data);

        // Ensure we have enough data for the entire list
        let available_data = reader.data().len();
        if available_data < total_size {
            // Try to load more data
            let loaded_data = reader
                .load_data(total_size - available_data)
                .map_err(|error| {
                    ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
                })?;

            // Check if we have enough data now
            if available_data + loaded_data < total_size {
                return Err(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                    required_data: total_size,
                    available_data: available_data + loaded_data,
                });
            }
        }

        // Get the final data guard with all required data
        Ok((reader.data(), total_size))
    }

    fn cast(data: &'buf [u8], offset: usize) -> Result<Self, ZeroCopyRingBufferReadableError> {
        // Unwrap not good >:(
        Ok(Self::new(data).unwrap())
    }

    /*
    fn read(
        reader: &'buf mut R,
    ) -> Result<(DataGuard<'buf, R>, Self), ZeroCopyRingBufferReadableError> {
        // Check if there is enough data for the header
        const HEADER_SIZE: usize = size_of::<I32ListHeader>();
        let available_data = reader.data().len();

        if available_data < HEADER_SIZE {
            // Try to load more data
            let loaded_data = reader
                .load_data(HEADER_SIZE - available_data)
                .map_err(|error| {
                    ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
                })?;

            // Check if we have enough data now
            if available_data + loaded_data < HEADER_SIZE {
                return Err(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                    required_data: HEADER_SIZE,
                    available_data: available_data + loaded_data,
                });
            }
        }

        // Get temporary access to data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[..HEADER_SIZE];
        let header = unsafe { &*(header_data.as_ptr() as *const I32ListHeader) };

        // Calculate the total size needed based on header information
        let element_count = header.element_count as usize;
        let total_size = HEADER_SIZE + element_count * size_of::<i32>();

        // Drop the temporary data access before potentially loading more
        drop(temp_data);

        // Ensure we have enough data for the entire list
        let available_data = reader.data().len();
        if available_data < total_size {
            // Try to load more data
            let loaded_data = reader
                .load_data(total_size - available_data)
                .map_err(|error| {
                    ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
                })?;

            // Check if we have enough data now
            if available_data + loaded_data < total_size {
                return Err(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                    required_data: total_size,
                    available_data: available_data + loaded_data,
                });
            }
        }

        // Get the final data guard with all required data
        let data_guard = reader.data();

        // Create the I32List that references the buffer data
        let list = I32List { data: &data_guard[..total_size] };

        // Return the TypedDataGuard that owns the I32List wrapper
        Ok((data_guard, list))
    }

    fn read_multiple(
        reader: &'buf mut R,
        count: usize,
    ) -> Result<TypedMultiDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
        todo!()
    }

    fn extend_read_multiple(
        reader: &'buf mut R,
        total_count: usize,
        existing_guard: TypedMultiDataGuard<'buf, R, Self>,
    ) -> Result<TypedMultiDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
        todo!()
    }
    */
}

/*
impl<'a, R: ZeroCopyRingBufferReader> ZeroCopyRingBufferReadable<'a, R> for TestReadable<'a> {
    fn read(buffer: &'a mut R) -> Result<(Self, DataGuard<'a, R>), ZeroCopyRingBufferReaderTypedReadError>
    where
        Self: 'a
    {
        let available_data = buffer.data().len();

        if available_data < size_of::<TestReadableHeader>() {
            let missing_data = size_of::<Self>() - available_data;
            buffer.load_data(missing_data).map_err(|error| {
                ZeroCopyRingBufferReaderTypedReadError::ZeroCopyRingBufferReaderError(error)
            })?;
        }

        let available_data = buffer.data().len();

        if available_data < size_of::<TestReadableHeader>() {
            Err(ZeroCopyRingBufferReaderTypedReadError::MissingData {
                type_size: size_of::<TestReadableHeader>(),
                available_data,
            })?;
        }

        let header_data = &buffer.data()[..size_of::<Self>()];
        let test_header = unsafe { &*(header_data.as_ptr() as *const TestReadableHeader) };

        // Since the pcie40 produces non TestReadable data the data size will be assumed to be 4
        let body_data_len = 4_usize;

        let available_data = buffer.data().len() - size_of::<TestReadableHeader>();

        if available_data < body_data_len {
            let missing_data = size_of::<Self>() - available_data;
            buffer.load_data(missing_data).map_err(|error| {
                ZeroCopyRingBufferReaderTypedReadError::ZeroCopyRingBufferReaderError(error)
            })?;
        }

        let available_data = buffer.data().len() - size_of::<TestReadableHeader>();

        if available_data < body_data_len {
            Err(ZeroCopyRingBufferReaderTypedReadError::MissingData {
                type_size: body_data_len,
                available_data,
            })?;
        }

        let data_guard = buffer.data();
        let body_data = &data_guard[size_of::<TestReadableHeader>()..];

        let test_readable = TestReadable {
            header: test_header,
            data: body_data,
        };

        Ok((test_readable, data_guard))
    }
}
*/

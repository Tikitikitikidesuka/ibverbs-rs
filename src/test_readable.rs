use crate::zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReadable, ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
    ZeroCopyRingBufferReaderTypedReadError,
};

#[repr(C, packed)]
#[derive(Debug)]
pub struct TestReadable {
    a: i32,
    b: i32,
    c: i32,
    d: i32,
}

impl<R: ZeroCopyRingBufferReader> ZeroCopyRingBufferReadable<R> for TestReadable {
    fn read<'a>(buffer: &mut R) -> Result<&'a Self, ZeroCopyRingBufferReaderTypedReadError> {
        let data_available = buffer.data().len();

        if data_available < size_of::<Self>() {
            let missing_data = size_of::<Self>() - data_available;
            buffer.load_data(missing_data).map_err(|error| {
                ZeroCopyRingBufferReaderTypedReadError::ZeroCopyRingBufferReaderError(error)
            })?;
        }

        let available_data = buffer.data().len();

        if available_data < size_of::<TestReadable>() {
            Err(ZeroCopyRingBufferReaderTypedReadError::MissingData {
                type_size: size_of::<Self>(),
                available_data,
            })?;
        }

        let data = &buffer.data()[..size_of::<Self>()];

        let test_readable = unsafe { &*(data.as_ptr() as *const Self) };

        Ok(test_readable)
    }
}

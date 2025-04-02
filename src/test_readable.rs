use crate::zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReadable, ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
    ZeroCopyRingBufferReaderTypedReadError,
};

#[repr(C, packed)]
#[derive(Debug)]
pub struct TestReadable<'a> {
    header: &'a TestReadableHeader,
    data: &'a [u8],
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct TestReadableHeader {
    a: i32,
    b: i32,
    c: i32,
}

impl<R: ZeroCopyRingBufferReader> ZeroCopyRingBufferReadable<R> for TestReadable<'_> {
    fn read<'a>(buffer: &mut R) -> Result<&'a Self, ZeroCopyRingBufferReaderTypedReadError> {
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

        let body_data = &buffer.data()[size_of::<TestReadableHeader>()..];

        let test_readable = TestReadable {
            header: test_header,
            data: body_data,
        };

        Ok(&test_readable)
    }
}

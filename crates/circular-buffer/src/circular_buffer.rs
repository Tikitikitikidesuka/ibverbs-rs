use std::error::Error;

/// Defines a zero-copy reader over a single-producer single-consumer circular buffer.
///
/// All rusteb readers for circular buffers implement this trait.
/// See the reader for _PCIe40 readout cards_ in the [pcie40](../pcie40) crate;
/// and the reader for _interprocess communications over shared memory buffers_ in the [shared-memory-buffer](../shared-memory-buffer) crate.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// # #[cfg(feature = "mock-buffers")] {
/// # use circular_buffer::mock_buffers::{MockAliasedBuffer, MockAliasedBufferReader, MockAliasedBufferWriter};
/// # use circular_buffer::{CircularBufferReader, CircularBufferWriter};
/// # let mut buffer = MockAliasedBuffer::new(8, 0).unwrap();
/// # let mut reader = MockAliasedBufferReader::new(&mut buffer).unwrap();
/// # let mut writer = MockAliasedBufferWriter::new(&mut buffer).unwrap();
/// # writer.writable_region().copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7]);
/// # writer.advance_write_pointer(4).unwrap();
/// // Start with readable region [0, 1, 2, 3]
/// println!("{:?}", reader.readable_region()); // [0, 1, 2, 3]
/// reader.advance_read_pointer(2).unwrap(); // Discard two bytes
/// println!("{:?}", reader.readable_region()); // [2, 3]
/// # }
/// ```
pub trait CircularBufferReader {
    type AdvanceStatus;
    type AdvanceError: Error;
    type ReadableRegion<'buf_ref>
    where
        Self: 'buf_ref;
    type ReadableRegionError: Error;

    fn advance_read_pointer(
        &mut self,
        bytes: usize,
    ) -> Result<Self::AdvanceStatus, Self::AdvanceError>;
    fn readable_region(&self) -> Result<Self::ReadableRegion<'_>, Self::ReadableRegionError>;
}

pub trait CircularBufferWriter {
    type AdvanceStatus;
    type AdvanceError: Error;
    type WriteableRegion<'buf_ref>
    where
        Self: 'buf_ref;
    type WriteableRegionError: Error;

    fn advance_write_pointer(
        &mut self,
        bytes: usize,
    ) -> Result<Self::AdvanceStatus, Self::AdvanceError>;
    fn writable_region(&mut self) -> Result<Self::WriteableRegion<'_>, Self::WriteableRegionError>;
}

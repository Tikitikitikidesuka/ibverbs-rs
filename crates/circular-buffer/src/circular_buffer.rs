/// Single-producer single-consumer zero copy reader for circular buffers
///
/// `CircularBufferReader` defines how a type will read data from a single-producer single-consumer
/// circular buffer it has access to in a zero copy manner, meaning, the return should be a reference
/// to the data in the circular buffer itself.
///
/// All LHCb Event Builder types for reading from circular buffers implement this trait.
/// See the [pcie40](../pcie40) and [shared-memory-buffer](../shared-memory-buffer) crates for implementations of this trait
/// for reading from _PCIe40 readout cards_ and _interprocess shared memory buffers_ respectively.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// # #[cfg(feature = "mock-buffers")] {
/// // TODO: Move the mock buffers to this crate and use them in the examples
/// # }
/// ```
pub trait CircularBufferReader {
    type AdvanceResult;
    type ReadableRegionResult<'a>
    where
        Self: 'a;

    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult;
    fn readable_region(&self) -> Self::ReadableRegionResult<'_>;
}

pub trait CircularBufferWriter {
    type AdvanceResult;
    type WriteableRegionResult<'a>
    where
        Self: 'a;

    fn advance_write_pointer(&mut self, bytes: usize) -> Self::AdvanceResult;
    fn writable_region(&mut self) -> Self::WriteableRegionResult<'_>;
}

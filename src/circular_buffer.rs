pub trait CircularBufferReader {
    type AdvanceResult;
    type ReadableRegionResult<'a> where Self: 'a;

    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult;
    fn readable_region(&self) -> Self::ReadableRegionResult<'_>;
}

pub trait CircularBufferWriter {
    type AdvanceResult;
    type WriteableRegionResult<'a> where Self: 'a;

    fn advance_write_pointer(&mut self, bytes: usize) -> Self::AdvanceResult;
    fn writable_region(&mut self) -> Self::WriteableRegionResult<'_>;
}
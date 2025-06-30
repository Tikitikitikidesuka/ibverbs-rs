use crate::pcie40::bindings::*;
use crate::pcie40::stream::locked_stream::PCIe40LockedStream;
use crate::pcie40::stream::stream::PCIe40StreamError;
use log::{debug, trace};
use std::mem::ManuallyDrop;

pub struct PCIe40MappedStream<'a> {
    locked_stream: ManuallyDrop<PCIe40LockedStream>,
    buffer: &'a [u8],
}

impl Drop for PCIe40MappedStream<'_> {
    fn drop(&mut self) {
        trace!(
            "Drop called on PCIe40MappedBuffer for device {} stream {}",
            self.locked_stream.stream.device_id, self.locked_stream.stream.stream_type
        );
        self.ref_unmap_buffer();
        unsafe {
            ManuallyDrop::drop(&mut self.locked_stream);
        }
    }
}

impl<'a> PCIe40MappedStream<'a> {
    pub(super) fn new(locked_stream: PCIe40LockedStream, buffer: &'a [u8]) -> Self {
        Self {
            locked_stream: ManuallyDrop::new(locked_stream),
            buffer,
        }
    }

    fn ref_unmap_buffer(&mut self) {
        debug!(
            "Unmapping buffer for stream {} on device {}",
            self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id
        );

        trace!(
            "Calling p40_stream_unmap({}, {:p})",
            self.locked_stream.stream.stream_fd,
            self.buffer.as_ptr() as *mut std::os::raw::c_void
        );
        unsafe {
            p40_stream_unmap(
                self.locked_stream.stream.stream_fd,
                self.buffer.as_ptr() as *mut std::os::raw::c_void,
            )
        }
        debug!(
            "Successfully unmapped buffer for stream {} on device {}",
            self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id
        );
    }

    fn unmap_buffer(mut self) -> PCIe40LockedStream {
        self.ref_unmap_buffer();

        // Take ownership of the locked stream avoiding Drop impl restriction
        let locked_stream =
            unsafe { ManuallyDrop::into_inner(std::ptr::read(&self.locked_stream)) };
        // Forget self to prevent Drop from running
        std::mem::forget(self);

        locked_stream
    }
}

impl PCIe40MappedStream<'_> {
    /// Returns a slice to the whole buffer.
    /// The real buffer is half the size of the returned one. It is aliased in
    ///virtual memory to always allow contiguous access to its contents.
    ///
    /// # Safety
    /// The buffer's data might change due to the DMA access from the card, so it is not really immutable
    pub unsafe fn data(&self) -> &[u8] {
        self.buffer
    }

    /// Gets the read offset from the card
    pub fn get_read_offset(&self) -> Result<usize, PCIe40StreamError> {
        let offset =
            unsafe { p40_stream_get_host_buf_read_off(self.locked_stream.stream.stream_fd) };

        if offset < 0 {
            return Err(PCIe40StreamError::StreamReadError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: "Unable to get buffer read offset".to_string(),
            });
        }

        Ok(offset as usize)
    }

    /// Gets the write offset from the card
    pub fn get_write_offset(&self) -> Result<usize, PCIe40StreamError> {
        let offset =
            unsafe { p40_stream_get_host_buf_write_off(self.locked_stream.stream.stream_fd) };

        if offset < 0 {
            return Err(PCIe40StreamError::StreamReadError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: "Unable to get buffer write offset".to_string(),
            });
        }

        Ok(offset as usize)
    }

    /// Moves the read offset by exactly `offset` bytes. Otherwise, it returns a stream error.
    pub fn move_read_offset(&mut self, offset: usize) -> Result<(), PCIe40StreamError> {
        // Get read region size
        let available =
            unsafe { p40_stream_get_host_buf_bytes_used(self.locked_stream.stream.stream_fd) };

        // Check enough available bytes
        if available < 0 || (available as usize) < offset {
            return Err(PCIe40StreamError::StreamReadError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: format!(
                    "Cannot move read offset. Requested to move it {offset} bytes but only {available} available"
                ),
            });
        }

        // If enough available data, move read offset on the card
        unsafe { p40_stream_free_host_buf_bytes(self.locked_stream.stream.stream_fd, offset) };

        Ok(())
    }

    pub fn available_bytes(&self) -> Result<usize, PCIe40StreamError> {
        let available =
            unsafe { p40_stream_get_host_buf_bytes_used(self.locked_stream.stream.stream_fd) };

        if available < 0 {
            return Err(PCIe40StreamError::StreamReadError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: "Unable to get available bytes on buffer".to_string(),
            });
        }

        Ok(available as usize)
    }
}

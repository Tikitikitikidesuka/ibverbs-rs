use crate::pcie40::bindings::*;
use crate::pcie40::pcie40_stream::locked_stream::PCIe40LockedStream;
use crate::pcie40::pcie40_stream::stream::{PCIe40Stream, PCIe40StreamError};
use log::{debug, error, trace};
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
        let locked_stream = unsafe { ManuallyDrop::into_inner(std::ptr::read(&self.locked_stream)) };
        // Forget self to prevent Drop from running
        std::mem::forget(self);

        locked_stream
    }
}

impl PCIe40MappedStream<'_> {
    pub unsafe fn data(&self) -> &[u8] {
        trace!(
            "Accessing buffer data of size {} for stream {} on device {}",
            self.buffer.len(),
            self.locked_stream.stream.stream_type,
            self.locked_stream.stream.device_id
        );
        self.buffer
    }

    pub fn get_read_offset(&self) -> Result<usize, PCIe40StreamError> {
        trace!(
            "Getting buffer read offset for stream {} on device {}",
            self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id
        );

        trace!(
            "Calling p40_stream_get_host_buf_read_off({})",
            self.locked_stream.stream.stream_fd
        );
        let offset =
            unsafe { p40_stream_get_host_buf_read_off(self.locked_stream.stream.stream_fd) };
        trace!("p40_stream_get_host_buf_read_off returned {}", offset);

        if offset < 0 {
            error!(
                "Failed to get buffer read offset for stream {} on device {}",
                self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id,
            );
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: "Unable to get buffer read offset".to_string(),
            })
        } else {
            debug!(
                "Buffer read offset for stream {} on device {}: {}",
                self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id, offset
            );
            Ok(offset as usize)
        }
    }

    pub fn available_bytes(&self) -> Result<usize, PCIe40StreamError> {
        trace!(
            "Getting available bytes of mapped buffer for stream {} on device {}",
            self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id
        );

        trace!(
            "Calling p40_stream_get_host_buf_read_off({})",
            self.locked_stream.stream.stream_fd
        );
        let available =
            unsafe { p40_stream_get_host_buf_bytes_used(self.locked_stream.stream.stream_fd) };
        trace!("p40_stream_get_host_buf_bytes_used returned {}", available);

        if available < 0 {
            error!(
                "Failed to get available bytes on buffer for stream {} on device {}",
                self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id
            );
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: "Unable to get available bytes on buffer".to_string(),
            })
        } else {
            debug!(
                "Available bytes for stream {} on device {}: {}",
                self.locked_stream.stream.stream_type,
                self.locked_stream.stream.device_id,
                available
            );
            Ok(available as usize)
        }
    }

    pub fn get_write_offset(&self) -> Result<usize, PCIe40StreamError> {
        trace!(
            "Getting buffer write offset for stream {} on device {}",
            self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id
        );

        trace!(
            "Calling p40_stream_get_host_buf_write_off({})",
            self.locked_stream.stream.stream_fd
        );
        let offset =
            unsafe { p40_stream_get_host_buf_write_off(self.locked_stream.stream.stream_fd) };
        trace!("p40_stream_get_host_buf_write_off returned {}", offset);

        if offset < 0 {
            error!(
                "Failed to get buffer write offset for stream {} on device {}",
                self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id,
            );
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: "Unable to get buffer write offset".to_string(),
            })
        } else {
            debug!(
                "Buffer write offset for stream {} on device {}: {}",
                self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id, offset
            );
            Ok(offset as usize)
        }
    }

    pub fn move_read_offset(&mut self, offset: usize) -> Result<usize, PCIe40StreamError> {
        trace!(
            "Moving buffer read offset for stream {} on device {}",
            self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id
        );

        trace!(
            "Calling p40_stream_free_host_buf_bytes({}, {})",
            self.locked_stream.stream.stream_fd, offset
        );
        let offset =
            unsafe { p40_stream_free_host_buf_bytes(self.locked_stream.stream.stream_fd, offset) };
        trace!("p40_stream_free_host_buf_bytes returned {}", offset);

        if offset < 0 {
            error!(
                "Failed to move buffer read offset for stream {} on device {}",
                self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id,
            );
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.locked_stream.stream.device_id,
                stream_type: self.locked_stream.stream.stream_type,
                info: "Unable to move buffer read offset".to_string(),
            })
        } else {
            debug!(
                "Buffer read offset for stream {} on device {} moved {} bytes",
                self.locked_stream.stream.stream_type, self.locked_stream.stream.device_id, offset
            );
            Ok(offset as usize)
        }
    }
}

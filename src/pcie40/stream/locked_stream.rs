use crate::pcie40::bindings::*;
use crate::pcie40::stream::mapped_stream::PCIe40MappedStream;
use crate::pcie40::stream::stream::{PCIe40Stream, PCIe40StreamError};
use std::mem::ManuallyDrop;
use std::slice;
use tracing::instrument;
use tracing::{debug, trace, warn};

pub struct PCIe40LockedStream {
    pub(super) stream: ManuallyDrop<PCIe40Stream>,
}

impl Drop for PCIe40LockedStream {
    #[instrument(skip_all, fields(
        device_id = self.stream.device_id(),
        stream_type = ?self.stream.stream_type()
    ))]
    fn drop(&mut self) {
        debug!(
            "Drop called on PCIe40LockedStream for device {} stream {}",
            self.stream.device_id(),
            self.stream.stream_type()
        );
        if let Err(e) = self.ref_unlock() {
            warn!("Failed to unlock stream during Drop: {}", e);
        }
        unsafe {
            ManuallyDrop::drop(&mut self.stream);
        }
    }
}

impl PCIe40LockedStream {
    pub(super) fn new(stream: PCIe40Stream) -> Self {
        Self {
            stream: ManuallyDrop::new(stream),
        }
    }

    #[instrument(skip_all, fields(
        device_id = self.stream.device_id(),
        stream_type = ?self.stream.stream_type()
    ))]
    pub fn unlock(mut self) -> Result<PCIe40Stream, PCIe40StreamError> {
        debug!(
            "Unlocking stream {} on device {}",
            self.stream.stream_type(),
            self.stream.device_id()
        );
        self.ref_unlock()?;

        debug!("Taking ownership of stream to manually drop");
        let stream = unsafe { ManuallyDrop::into_inner(std::ptr::read(&self.stream)) };
        debug!("Forgetting self to prevent Drop from running");
        std::mem::forget(self);

        Ok(stream)
    }

    fn ref_unlock(&mut self) -> Result<(), PCIe40StreamError> {
        debug!(
            "Unlocking stream {} on device {}",
            self.stream.stream_type(),
            self.stream.device_id()
        );

        trace!("Calling p40_stream_unlock({})", self.stream.stream_fd);
        let c_result = unsafe { p40_stream_unlock(self.stream.stream_fd) };
        trace!("p40_stream_unlock returned {}", c_result);

        match c_result.cmp(&0) {
            std::cmp::Ordering::Equal => {
                debug!(
                    "Successfully unlocked stream {} on device {}",
                    self.stream.stream_type(),
                    self.stream.device_id()
                );
                Ok(())
            }
            std::cmp::Ordering::Greater => {
                debug!(
                    "Failed to unlock stream {} on device {} (locked by process {})",
                    self.stream.stream_type(),
                    self.stream.device_id(),
                    c_result
                );
                Err(PCIe40StreamError::StreamWriteError {
                    device_id: self.stream.device_id(),
                    stream_type: self.stream.stream_type(),
                    info: format!(
                        "Failed to unlock stream. It is locked by another process (pid: {c_result})"
                    ),
                })
            }
            std::cmp::Ordering::Less => {
                debug!(
                    "Error writing unlock for stream {} on device {}",
                    self.stream.stream_type(),
                    self.stream.device_id()
                );
                Err(PCIe40StreamError::StreamWriteError {
                    device_id: self.stream.device_id(),
                    stream_type: self.stream.stream_type(),
                    info: "Unable to write unlock".to_string(),
                })
            }
        }
    }

    #[instrument(skip_all, fields(
        device_id = self.stream.device_id(),
        stream_type = ?self.stream.stream_type()
    ))]
    pub fn reset_flush(&mut self) -> Result<(), PCIe40StreamError> {
        debug!("Flushing stream's memory");

        trace!("Calling p40_stream_reset_flush({})", self.stream.stream_fd);
        let result = unsafe { p40_stream_reset_flush(self.stream.stream_fd) };
        if result != 0 {
            warn!("Failed to flush stream: {}", result);
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream.device_id,
                stream_type: self.stream.stream_type,
                info: "Could not flush stream".to_string(),
            })
        } else {
            debug!("Successfully flushed stream");
            Ok(())
        }
    }

    #[instrument(skip_all, fields(
        device_id = self.stream.device_id(),
        stream_type = ?self.stream.stream_type()
    ))]
    pub fn reset_logic(&mut self) -> Result<(), PCIe40StreamError> {
        debug!("Resetting logic on stream");
        trace!("Calling p40_stream_reset_logic({})", self.stream.stream_fd);
        let result = unsafe { p40_stream_reset_logic(self.stream.stream_fd) };
        if result != 0 {
            warn!("Failed to reset logic on stream: {}", result);
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream.device_id,
                stream_type: self.stream.stream_type,
                info: "Could not reset logic on stream".to_string(),
            })
        } else {
            debug!("Successfully reset logic on stream");
            Ok(())
        }
    }

    #[instrument(skip_all, fields(
        device_id = self.stream.device_id(),
        stream_type = ?self.stream.stream_type()
    ))]
    pub fn map_buffer<'a>(self) -> Result<PCIe40MappedStream<'a>, PCIe40StreamError> {
        debug!(
            "Mapping buffer for stream {} on device {}",
            self.stream.stream_type(),
            self.stream.device_id()
        );

        trace!("Calling p40_stream_map({})", self.stream.stream_fd);
        let buff_ptr = unsafe { p40_stream_map(self.stream.stream_fd) };
        trace!("p40_stream_map returned {:p}", buff_ptr);

        if buff_ptr.is_null() {
            warn!(
                "Failed to map buffer for stream {} on device {}: null pointer",
                self.stream.stream_type(),
                self.stream.device_id()
            );
            return Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream.device_id(),
                stream_type: self.stream.stream_type(),
                info: "Failed to map buffer. Null pointer".to_string(),
            });
        }

        trace!(
            "Calling p40_stream_get_host_buf_bytes({})",
            self.stream.stream_fd
        );
        let buff_size = unsafe { p40_stream_get_host_buf_bytes(self.stream.stream_fd) };
        trace!("p40_stream_get_host_buf_bytes returned {}", buff_size);

        if buff_size <= 0 {
            warn!(
                "Failed to map buffer for stream {} on device {}: invalid buffer size {}",
                self.stream.stream_type(),
                self.stream.device_id(),
                buff_size
            );
            return Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream.device_id(),
                stream_type: self.stream.stream_type(),
                info: format!("Failed to map buffer. Invalid buffer size: {}", buff_size),
            });
        }

        debug!(
            "Successfully mapped buffer of size {} bytes for stream {} on device {}",
            buff_size,
            self.stream.stream_type(),
            self.stream.device_id()
        );

        Ok(PCIe40MappedStream::new(self, unsafe {
            slice::from_raw_parts(buff_ptr as *const u8, buff_size as usize)
        }))
    }
}

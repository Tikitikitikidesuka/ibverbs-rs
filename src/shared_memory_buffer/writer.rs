use crate::shared_memory_buffer::buffer_backend::SharedMemoryWriteBuffer;
use crate::shared_memory_buffer::buffer_status::CircularBufferStatus;
use log::{debug, error, info, trace};
use thiserror::Error;

pub struct SharedMemoryBufferWriter {
    buffer: SharedMemoryWriteBuffer,
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error(
        "Insufficient space on buffer. Available: {available} bytes; Requested: {requested} bytes"
    )]
    InsufficientSpace { available: usize, requested: usize },
    #[error(
        "Write would wraparound: Available contiguous: {available_contiguous} bytes; Requested: {requested} bytes"
    )]
    WouldWrap {
        available_contiguous: usize,
        requested: usize,
    },
    #[error(
        "Due to the pointer representation on this buffer, the minimum addressable amount of data is two bytes. Data written must have a length divisible by 2 bytes."
    )]
    Not2ByteAligned,
}

impl SharedMemoryBufferWriter {
    pub fn new(write_buffer: SharedMemoryWriteBuffer) -> Self {
        info!("Creating new SharedMemoryBufferWriter for buffer of size {} bytes",
              write_buffer.size());

        debug!("Writer initialized with buffer size: {} bytes", write_buffer.size());

        Self {
            buffer: write_buffer,
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, WriteError> {
        let data_len = buf.len();
        debug!("Write operation requested: {} bytes", data_len);

        if data_len == 0 {
            trace!("Empty write request, returning immediately");
            return Ok(0);
        }

        trace!("Validating alignment requirement for write of {} bytes", data_len);
        if data_len % 2 != 0 {
            error!("Write rejected: {} bytes is not 2-byte aligned", data_len);
            return Err(WriteError::Not2ByteAligned);
        }
        trace!("Alignment validation passed for {} bytes", data_len);

        // Read statuses ONCE
        debug!("Reading buffer status for write operation");
        let write_status = self.buffer.write_status();
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        trace!("Buffer status snapshot: write_ptr={}, write_wrap={}, read_ptr={}, read_wrap={}, buffer_size={}",
               write_status.ptr(), write_status.wrap(),
               read_status.ptr(), read_status.wrap(), buffer_size);

        // Calculate spaces using the captured statuses
        trace!("Calculating available space to write");
        let available =
            CircularBufferStatus::buffer_available_to_write(write_status, read_status, buffer_size);
        debug!("Available space to write: {} bytes (requested: {} bytes)", available, data_len);

        if data_len > available {
            error!("Insufficient space for write: available={} bytes, requested={} bytes",
                   available, data_len);
            return Err(WriteError::InsufficientSpace {
                available,
                requested: data_len,
            });
        }

        trace!("Calculating tail free space for potential wrapping");
        let tail_space =
            CircularBufferStatus::buffer_tail_free_space(write_status, read_status, buffer_size);
        debug!("Tail free space: {} bytes", tail_space);

        // Use the consistent write position from our snapshot
        let write_pos = write_status.ptr() as usize;
        trace!("Write will begin at position: {} (0x{:x})", write_pos, write_pos);

        let buffer_slice = unsafe { self.buffer.as_slice_mut() };
        trace!("Got mutable buffer slice for write operation");

        if data_len <= tail_space {
            // Simple case: fits without wrapping
            debug!("Write fits without wrapping: {} bytes at position {}", data_len, write_pos);
            trace!("Copying {} bytes to buffer[{}..{}]", data_len, write_pos, write_pos + data_len);

            buffer_slice[write_pos..write_pos + data_len].copy_from_slice(buf);

            debug!("Successfully wrote {} bytes at position {} without wrapping", data_len, write_pos);
        } else {
            // Write and wrap
            debug!("Write requires wrapping: {} bytes total, {} bytes at tail, {} bytes at head",
                   data_len, tail_space, data_len - tail_space);

            trace!("Writing {} bytes to tail: buffer[{}..{}]", tail_space, write_pos, write_pos + tail_space);
            buffer_slice[write_pos..write_pos + tail_space].copy_from_slice(&buf[..tail_space]);

            let remaining = data_len - tail_space;
            trace!("Writing remaining {} bytes to head: buffer[0..{}]", remaining, remaining);
            buffer_slice[..remaining].copy_from_slice(&buf[tail_space..]);

            debug!("Successfully wrote {} bytes with wrapping: {} at tail + {} at head",
                   data_len, tail_space, remaining);
        }

        // Update write pointer based on our snapshot
        trace!("Updating write pointer from current position");
        let new_write_status = write_status.add(data_len, buffer_size);
        trace!("New write status: ptr={}, wrap={} (added {} bytes)",
               new_write_status.ptr(), new_write_status.wrap(), data_len);

        self.buffer.set_write_status(new_write_status);
        debug!("Updated write pointer: new_pos={}, wrap_bit={}",
               new_write_status.ptr(), new_write_status.wrap());

        info!("Write operation completed successfully: {} bytes written", data_len);
        Ok(data_len)
    }

    pub fn write_no_wrapping(&mut self, buf: &[u8]) -> Result<usize, WriteError> {
        let data_len = buf.len();
        debug!("No-wrap write operation requested: {} bytes", data_len);

        if data_len == 0 {
            trace!("Empty no-wrap write request, returning immediately");
            return Ok(0);
        }

        // Read statuses ONCE
        debug!("Reading buffer status for no-wrap write operation");
        let write_status = self.buffer.write_status();
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        trace!("Buffer status snapshot for no-wrap: write_ptr={}, write_wrap={}, read_ptr={}, read_wrap={}, buffer_size={}",
               write_status.ptr(), write_status.wrap(),
               read_status.ptr(), read_status.wrap(), buffer_size);

        // Calculate tail space with our snapshot
        trace!("Calculating tail free space for no-wrap write");
        let tail_space =
            CircularBufferStatus::buffer_tail_free_space(write_status, read_status, buffer_size);
        debug!("Tail free space: {} bytes (requested: {} bytes)", tail_space, data_len);

        if data_len > tail_space {
            error!("No-wrap write would require wrapping: available_contiguous={} bytes, requested={} bytes",
                   tail_space, data_len);
            return Err(WriteError::WouldWrap {
                available_contiguous: tail_space,
                requested: data_len,
            });
        }

        // Use consistent write position
        let write_pos = write_status.ptr() as usize;
        trace!("No-wrap write will begin at position: {} (0x{:x})", write_pos, write_pos);

        let buffer_slice = unsafe { self.buffer.as_slice_mut() };
        trace!("Got mutable buffer slice for no-wrap write operation");

        trace!("Copying {} bytes to buffer[{}..{}] (no wrapping)", data_len, write_pos, write_pos + data_len);
        buffer_slice[write_pos..write_pos + data_len].copy_from_slice(buf);
        debug!("Successfully wrote {} bytes at position {} without wrapping", data_len, write_pos);

        // Update based on snapshot
        trace!("Updating write pointer from current position (no-wrap)");
        let new_write_status = write_status.add(data_len, buffer_size);
        trace!("New write status (no-wrap): ptr={}, wrap={} (added {} bytes)",
               new_write_status.ptr(), new_write_status.wrap(), data_len);

        self.buffer.set_write_status(new_write_status);
        debug!("Updated write pointer (no-wrap): new_pos={}, wrap_bit={}",
               new_write_status.ptr(), new_write_status.wrap());

        info!("No-wrap write operation completed successfully: {} bytes written", data_len);
        Ok(data_len)
    }
}
use crate::shared_memory_buffer::buffer_backend::SharedMemoryWriteBuffer;
use crate::shared_memory_buffer::buffer_status::CircularBufferStatus;
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
}

impl SharedMemoryBufferWriter {
    pub fn  new(write_buffer: SharedMemoryWriteBuffer) -> Self {
        Self {
            buffer: write_buffer,
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, WriteError> {
        let data_len = buf.len();
        if data_len == 0 {
            return Ok(0);
        }

        // Read statuses ONCE
        let write_status = self.buffer.write_status();
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        // Calculate spaces using the captured statuses
        let available =
            CircularBufferStatus::buffer_available_to_write(write_status, read_status, buffer_size);

        if data_len > available {
            return Err(WriteError::InsufficientSpace {
                available,
                requested: data_len,
            });
        }

        let tail_space =
            CircularBufferStatus::buffer_tail_free_space(write_status, read_status, buffer_size);

        // Use the consistent write position from our snapshot
        let write_pos = write_status.ptr() as usize;
        let buffer_slice = unsafe { self.buffer.as_slice_mut() };

        if data_len <= tail_space {
            // Simple case: fits without wrapping
            buffer_slice[write_pos..write_pos + data_len].copy_from_slice(buf);
        } else {
            // Write and wrap
            buffer_slice[write_pos..write_pos + tail_space].copy_from_slice(&buf[..tail_space]);

            let remaining = data_len - tail_space;
            buffer_slice[..remaining].copy_from_slice(&buf[tail_space..]);
        }

        // Update write pointer based on our snapshot
        let new_write_status = write_status.add(data_len, buffer_size);
        self.buffer.set_write_status(new_write_status);

        Ok(data_len)
    }

    pub fn write_no_wrapping(&mut self, buf: &[u8]) -> Result<usize, WriteError> {
        let data_len = buf.len();
        if data_len == 0 {
            return Ok(0);
        }

        // Read statuses ONCE
        let write_status = self.buffer.write_status();
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        // Calculate tail space with our snapshot
        let tail_space = CircularBufferStatus::buffer_tail_free_space(
            write_status,
            read_status,
            buffer_size
        );

        if data_len > tail_space {
            return Err(WriteError::WouldWrap {
                available_contiguous: tail_space,
                requested: data_len,
            });
        }

        // Use consistent write position
        let write_pos = write_status.ptr() as usize;
        let buffer_slice = unsafe { self.buffer.as_slice_mut() };

        buffer_slice[write_pos..write_pos + data_len]
            .copy_from_slice(buf);

        // Update based on snapshot
        let new_write_status = write_status.add(data_len, buffer_size);
        self.buffer.set_write_status(new_write_status);

        Ok(data_len)
    }
}

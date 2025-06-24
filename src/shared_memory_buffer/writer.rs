use crate::shared_memory_buffer::buffer_backend::SharedMemoryWriteBuffer;
use crate::shared_memory_buffer::buffer_element::BufferElement;
use crate::shared_memory_buffer::buffer_status::{CircularBufferStatus, PtrStatus};
use crate::utils;
use thiserror::Error;

pub struct SharedMemoryBufferWriter {
    buffer: SharedMemoryWriteBuffer,
    local_write_status: PtrStatus,
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

    #[error("Data is not aligned to 2^{alignment_2pow} bytes")]
    NotAligned { alignment_2pow: u8 },
}

impl SharedMemoryBufferWriter {
    pub fn new(buffer: SharedMemoryWriteBuffer) -> Self {
        let local_write_status = buffer.write_status();

        Self {
            buffer,
            local_write_status,
        }
    }

    // Writes an element to the ring buffer
    // It adds the necessary padding initialized to zero
    pub fn write_element<T: BufferElement>(&mut self, mut element: T) -> Result<(), WriteError> {
        // Read status *ONCE* to keep a stable state since they
        // might change during execution of the function.
        let write_status = self.local_write_status;
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        let tail_space =
            CircularBufferStatus::buffer_tail_free_space(write_status, read_status, buffer_size);

        if element.size() > tail_space {
            element.set_wrap()
        }

        unsafe { self.write_with_unsafe_padding(element.data()) }
    }

    // Writes an element to the ring buffer
    // SAFETY: When adding padding, it does not initialize it, just moves the write pointer
    pub unsafe fn unsafe_write_element<T: BufferElement>(
        &mut self,
        element: T,
    ) -> Result<(), WriteError> {
        unsafe { self.write_with_unsafe_padding(element.data()) }
    }

    // Writes data to the ring buffer.
    // The data must be aligned meaning it must have a length divisible by the alignment.
    pub fn write(&mut self, data: &[u8]) -> Result<(), WriteError> {
        self.check_alignment(data.len())?;
        self.unaligned_write(data)
    }

    // Writes data to the ring buffer with added padding to align it.
    fn write_with_padding(&mut self, data: &[u8]) -> Result<(), WriteError> {
        let aligned_length = utils::align_up_2pow(data.len(), self.buffer.alignment_2pow());
        let padding = aligned_length - data.len();
        self.check_2byte_alignment(aligned_length)?;

        self.unaligned_write(data)?;
        self.padding_write(0, padding)
    }

    // Writes data to the ring buffer with added padding to align it.
    // The padding is not writen, the write pointer is moved but the data remains the same.
    // SAFETY: The data marked as readable is uninitialized.
    unsafe fn write_with_unsafe_padding(&mut self, data: &[u8]) -> Result<(), WriteError> {
        let aligned_length = utils::align_up_2pow(data.len(), self.buffer.alignment_2pow());
        let padding = aligned_length - data.len();
        self.check_2byte_alignment(aligned_length)?;

        self.unaligned_write(data)?;
        unsafe { self.unsafe_padding_write(padding) }
    }

    // Writes data to the ring buffer contiguously.
    // The data must be aligned meaning it must have a length divisible by the alignment.
    // If there is still space on the ring buffer but not enough for the contiguous write,
    // it will fail and a `WriteError::WouldWrap` error will be returned.
    pub fn write_contiguous(&mut self, data: &[u8]) -> Result<(), WriteError> {
        self.check_alignment(data.len())?;

        // Read status *ONCE* to keep a stable state since they
        // might change during execution of the function.
        let write_status = self.local_write_status;
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        let tail_space =
            CircularBufferStatus::buffer_tail_free_space(write_status, read_status, buffer_size);

        if data.len() > tail_space {
            return Err(WriteError::WouldWrap {
                available_contiguous: tail_space,
                requested: data.len(),
            });
        }

        self.unaligned_write(data)
    }

    // Writes data to the ring buffer contiguously with added padding to align it.
    // If there is still space on the ring buffer but not enough for the contiguous write,
    // it will fail and a `WriteError::WouldWrap` error will be returned.
    pub fn write_contiguous_with_padding(&mut self, data: &[u8]) -> Result<(), WriteError> {
        todo!()
    }

    // Writes data to the ring buffer contiguously with added padding to align it.
    // The padding is not writen, the write pointer is moved but the data remains the same.
    // If there is still space on the ring buffer but not enough for the contiguous write,
    // it will fail and a `WriteError::WouldWrap` error will be returned.
    // SAFETY: The data marked as readable is uninitialized.
    pub unsafe fn write_contiguous_with_unsafe_padding(
        &mut self,
        data: &[u8],
    ) -> Result<(), WriteError> {
        todo!()
    }

    fn check_alignment(&self, length: usize) -> Result<(), WriteError> {
        if !utils::check_alignment_2pow(length, self.buffer.alignment_2pow()) {
            Err(WriteError::NotAligned {
                alignment_2pow: self.buffer.alignment_2pow(),
            })
        } else {
            self.check_2byte_alignment(length)
        }
    }

    // Check shared memory buffer's designed 2 byte alignment
    fn check_2byte_alignment(&self, length: usize) -> Result<(), WriteError> {
        if length % 2 != 0 {
            Err(WriteError::Not2ByteAligned)
        } else {
            Ok(())
        }
    }

    // Writes to the buffer without checking alignment.
    // Still checks the shared memory buffer restriction of 2 byte alignment.
    // Does not update the write pointer, this must be done explicitly with `update_write_status`.
    // This allows adding follow up write operations before making the writes visible to the consumers.
    fn unaligned_write(&mut self, data: &[u8]) -> Result<(), WriteError> {
        // Read status *ONCE* to keep a stable state since they
        // might change during execution of the function.
        let write_status = self.local_write_status;
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        let available =
            CircularBufferStatus::buffer_available_to_write(write_status, read_status, buffer_size);

        if data.len() > available {
            return Err(WriteError::InsufficientSpace {
                available,
                requested: data.len(),
            });
        }

        let tail_space =
            CircularBufferStatus::buffer_tail_free_space(write_status, read_status, buffer_size);

        let buffer_slice = unsafe { self.buffer.as_slice_mut() };

        if data.len() <= tail_space {
            // Simple case: data fits without wrapping
            buffer_slice[(write_status.ptr() as usize)..(write_status.ptr() as usize + data.len())]
                .copy_from_slice(data);
        } else {
            // Write with wrapping
            buffer_slice[(write_status.ptr() as usize)..(write_status.ptr() as usize + tail_space)]
                .copy_from_slice(&data[..tail_space]);

            let remaining = data.len() - tail_space;
            buffer_slice[..remaining].copy_from_slice(&data[tail_space..]);
        }

        // Update the local write status
        let new_write_status = write_status.add(data.len(), buffer_size);
        self.local_write_status = new_write_status;

        Ok(())
    }

    // Writes padding of the given value and length to the buffer.
    // Does not update the write pointer, this must be done explicitly with `update_write_status`.
    // This allows adding follow up write operations before making the writes visible to the consumers.
    fn padding_write(&mut self, value: u8, length: usize) -> Result<(), WriteError> {
        // Read status *ONCE* to keep a stable state since they
        // might change during execution of the function.
        let write_status = self.local_write_status;
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        let available =
            CircularBufferStatus::buffer_available_to_write(write_status, read_status, buffer_size);

        if length > available {
            return Err(WriteError::InsufficientSpace {
                available,
                requested: length,
            });
        }

        let tail_space =
            CircularBufferStatus::buffer_tail_free_space(write_status, read_status, buffer_size);

        let buffer_slice = unsafe { self.buffer.as_slice_mut() };

        if length <= tail_space {
            // Simple case: padding fits without wrapping
            buffer_slice[(write_status.ptr() as usize)..(write_status.ptr() as usize + length)]
                .fill(value);
        } else {
            // Write with wrapping
            buffer_slice[(write_status.ptr() as usize)..(write_status.ptr() as usize + tail_space)]
                .fill(value);

            let remaining = length - tail_space;
            buffer_slice[..remaining].fill(value);
        }

        // Update the local write status
        let new_write_status = write_status.add(length, buffer_size);
        self.local_write_status = new_write_status;

        Ok(())
    }

    // Moves the local write pointer but does not fill the buffer with actual data
    // Does not update the write pointer, this must be done explicitly with `update_write_status`.
    // This allows adding follow up write operations before making the writes visible to the consumers.
    // SAFETY: Leaves uninitialized data in the buffer
    unsafe fn unsafe_padding_write(&mut self, length: usize) -> Result<(), WriteError> {
        // Read status *ONCE* to keep a stable state since they
        // might change during execution of the function.
        let write_status = self.local_write_status;
        let read_status = self.buffer.read_status();
        let buffer_size = self.buffer.size();

        let available =
            CircularBufferStatus::buffer_available_to_write(write_status, read_status, buffer_size);

        if length > available {
            return Err(WriteError::InsufficientSpace {
                available,
                requested: length,
            });
        }

        // No actual buffer writes - just advance the write pointer
        // This leaves uninitialized/existing data in the buffer positions

        // Update the local write status
        let new_write_status = write_status.add(length, buffer_size);
        self.local_write_status = new_write_status;

        Ok(())
    }

    // Writes the local write pointer to the buffer, making all the previous writes visible to the consumers.
    fn commit_writes(&mut self) {
        self.buffer.set_write_status(self.local_write_status);
    }

    // Resets the previous uncommited write calls
    fn cancel_writes(&mut self) {
        self.local_write_status = self.buffer.write_status();
    }
}

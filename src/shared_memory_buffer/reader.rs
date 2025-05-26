use crate::shared_memory_buffer::buffer_backend::{
    SharedMemoryBufferNewError, SharedMemoryReadBuffer,
};
use crate::shared_memory_buffer::buffer_status::PtrStatus;
use crate::zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use log::{debug, error, info, trace};
use std::cmp::min;

pub struct SharedMemoryBufferReader {
    buffer: SharedMemoryReadBuffer,
    local_read_status: PtrStatus,
    local_write_status: PtrStatus,
}

impl SharedMemoryBufferReader {
    pub fn new(read_buffer: SharedMemoryReadBuffer) -> Self {
        info!("Creating new SharedMemoryBufferReader for buffer of size {} bytes",
              read_buffer.size());

        debug!("Reading initial buffer status from shared memory");
        let read_status = read_buffer.read_status();
        let write_status = read_buffer.write_status();

        debug!("Initial buffer state: read_ptr={}, read_wrap={}, write_ptr={}, write_wrap={}",
               read_status.ptr(), read_status.wrap(),
               write_status.ptr(), write_status.wrap());

        trace!("Initializing local status cache with shared memory values");
        let reader = Self {
            buffer: read_buffer,
            local_read_status: read_status,
            local_write_status: write_status,
        };

        let available = reader.available_bytes();
        info!("SharedMemoryBufferReader initialized: buffer_size={} bytes, available_data={} bytes",
              reader.buffer.size(), available);

        reader
    }
}

impl ZeroCopyRingBufferReader for SharedMemoryBufferReader {
    unsafe fn unsafe_data(&self) -> &[u8] {
        trace!("Getting unsafe data slice from buffer");

        let read_status = self.local_read_status;
        let write_status = self.local_write_status;
        let buffer_slice = self.buffer.as_slice();
        let buffer_len = buffer_slice.len();

        trace!("Current local status: read_ptr={}, read_wrap={}, write_ptr={}, write_wrap={}, buffer_len={}",
               read_status.ptr(), read_status.wrap(),
               write_status.ptr(), write_status.wrap(), buffer_len);

        if write_status.ptr() > read_status.ptr() {
            // Normal case: no wraparound, data is contiguous
            let start = read_status.ptr() as usize;
            let end = write_status.ptr() as usize;
            let slice_len = end - start;

            debug!("Returning contiguous data slice: [{}..{}] ({} bytes)", start, end, slice_len);
            trace!("No wraparound case: write_ptr ({}) > read_ptr ({})", write_status.ptr(), read_status.ptr());

            &buffer_slice[start..end]
        } else if write_status.ptr() <= read_status.ptr()
            && write_status.wrap() != read_status.wrap()
        {
            // Wraparound case: return only the contiguous portion from read_status to end
            // The wrapped portion (from start to write_status) will be available in the next call
            let start = read_status.ptr() as usize;
            let slice_len = buffer_len - start;

            debug!("Returning tail portion of wrapped data: [{}..{}] ({} bytes)",
                   start, buffer_len, slice_len);
            trace!("Wraparound case: write_ptr ({}) <= read_ptr ({}), different wrap bits ({} != {})",
                   write_status.ptr(), read_status.ptr(), write_status.wrap(), read_status.wrap());
            trace!("Wrapped portion [0..{}] will be available after tail is consumed", write_status.ptr());

            &buffer_slice[start..buffer_len]
        } else {
            trace!("No data available: empty buffer or fully consumed");
            debug!("Returning empty slice: write_ptr={}, read_ptr={}, write_wrap={}, read_wrap={}",
                   write_status.ptr(), read_status.ptr(), write_status.wrap(), read_status.wrap());
            &[]
        }
    }

    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Loading all new data from shared memory");

        let prev_write_status = self.local_write_status;
        trace!("Previous write status: ptr={}, wrap={}", prev_write_status.ptr(), prev_write_status.wrap());

        trace!("Reading current write status from shared memory");
        self.local_write_status = self.buffer.write_status();
        debug!("Updated local write status: ptr={}, wrap={}",
               self.local_write_status.ptr(), self.local_write_status.wrap());

        // Calculate bytes added, handling wraparound
        let bytes_added = if self.local_write_status.ptr() >= prev_write_status.ptr() {
            let bytes = (self.local_write_status.ptr() - prev_write_status.ptr()) as usize;
            trace!("No wraparound in write pointer: {} - {} = {} bytes added",
                   self.local_write_status.ptr(), prev_write_status.ptr(), bytes);
            bytes
        } else {
            // Wraparound occurred
            let tail_bytes = self.buffer.size() - prev_write_status.ptr() as usize;
            let head_bytes = self.local_write_status.ptr() as usize;
            let total_bytes = tail_bytes + head_bytes;

            debug!("Wraparound detected in write pointer: {} tail bytes + {} head bytes = {} total bytes added",
                   tail_bytes, head_bytes, total_bytes);
            trace!("Wraparound calculation: buffer_size({}) - prev_write_ptr({}) + new_write_ptr({}) = {}",
                   self.buffer.size(), prev_write_status.ptr(), self.local_write_status.ptr(), total_bytes);

            total_bytes
        };

        if bytes_added > 0 {
            info!("Loaded {} new bytes from shared memory", bytes_added);
        } else {
            trace!("No new data available to load");
        }

        Ok(bytes_added)
    }

    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding data: {} bytes requested", num_bytes);

        if num_bytes == 0 {
            trace!("Zero bytes requested for discard, returning immediately");
            return Ok(0);
        }

        let buffer_len = self.buffer.size();
        let available_bytes = self.available_bytes();
        let bytes_to_discard = min(num_bytes, available_bytes);

        debug!("Discard calculation: requested={}, available={}, will_discard={}",
               num_bytes, available_bytes, bytes_to_discard);

        if bytes_to_discard < num_bytes {
            debug!("Cannot discard all requested bytes: {} available < {} requested",
                   available_bytes, num_bytes);
        }

        trace!("Current read status before discard: ptr={}, wrap={}",
               self.local_read_status.ptr(), self.local_read_status.wrap());

        // Update read pointer with wraparound
        let old_read_status = self.local_read_status;
        self.local_read_status = self.local_read_status.add(bytes_to_discard, buffer_len);

        debug!("Updated local read status: ptr={}, wrap={} (advanced by {} bytes)",
               self.local_read_status.ptr(), self.local_read_status.wrap(), bytes_to_discard);

        if self.local_read_status.wrap() != old_read_status.wrap() {
            debug!("Read pointer wrapped around: wrap bit changed from {} to {}",
                   old_read_status.wrap(), self.local_read_status.wrap());
        }

        trace!("Updating shared memory read status");
        self.buffer.set_read_status(self.local_read_status);
        debug!("Successfully discarded {} bytes and updated shared memory", bytes_to_discard);

        Ok(bytes_to_discard)
    }

    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding all available data");

        let discarded_bytes = self.available_bytes();
        debug!("Will discard {} bytes (all available data)", discarded_bytes);

        if discarded_bytes == 0 {
            trace!("No data available to discard");
            return Ok(0);
        }

        trace!("Current status before discard_all: read_ptr={}, read_wrap={}, write_ptr={}, write_wrap={}",
               self.local_read_status.ptr(), self.local_read_status.wrap(),
               self.local_write_status.ptr(), self.local_write_status.wrap());

        debug!("Setting read status to match write status (discarding all data)");
        self.local_read_status = self.local_write_status;

        trace!("Updated read status to match write: ptr={}, wrap={}",
               self.local_read_status.ptr(), self.local_read_status.wrap());

        trace!("Updating shared memory read status");
        self.buffer.set_read_status(self.local_read_status);

        info!("Successfully discarded all {} bytes of available data", discarded_bytes);
        Ok(discarded_bytes)
    }

    fn alignment(&self) -> Result<Option<usize>, ZeroCopyRingBufferReaderError> {
        Ok(Some(1 << self.buffer.alignment_2pow()))
    }
}

impl SharedMemoryBufferReader {
    /// Calculate available bytes for reading, handling wraparound
    fn available_bytes(&self) -> usize {
        trace!("Calculating available bytes for reading");

        let buffer_len = self.buffer.size();
        let read_ptr = self.local_read_status.ptr();
        let write_ptr = self.local_write_status.ptr();
        let read_wrap = self.local_read_status.wrap();
        let write_wrap = self.local_write_status.wrap();

        trace!("Buffer state for calculation: buffer_len={}, read_ptr={}, write_ptr={}, read_wrap={}, write_wrap={}",
               buffer_len, read_ptr, write_ptr, read_wrap, write_wrap);

        let available = if write_ptr > read_ptr {
            let bytes = (write_ptr - read_ptr) as usize;
            trace!("No wraparound case: write_ptr ({}) > read_ptr ({}) = {} bytes",
                   write_ptr, read_ptr, bytes);
            bytes
        } else if write_ptr <= read_ptr && write_wrap != read_wrap {
            let tail_bytes = buffer_len - read_ptr as usize;
            let head_bytes = write_ptr as usize;
            let total_bytes = tail_bytes + head_bytes;

            trace!("Wraparound case: {} tail bytes + {} head bytes = {} total bytes",
                   tail_bytes, head_bytes, total_bytes);
            trace!("Wraparound condition: write_ptr ({}) <= read_ptr ({}) and different wrap bits ({} != {})",
                   write_ptr, read_ptr, write_wrap, read_wrap);

            total_bytes
        } else {
            trace!("Empty buffer case: pointers equal with same wrap bits or invalid state");
            trace!("Empty condition: write_ptr={}, read_ptr={}, write_wrap={}, read_wrap={}",
                   write_ptr, read_ptr, write_wrap, read_wrap);
            0
        };

        debug!("Available bytes calculation result: {} bytes", available);
        available
    }
}
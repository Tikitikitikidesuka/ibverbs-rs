use crate::backend::{SharedMemoryBuffer, SharedMemoryBufferError};
use crate::header::PtrStatus;
use crate::posix_advisory_file_lock::AdvisoryFileLock;
use crate::reader::SharedMemoryBufferReader;
use circular_buffer::{CircularBufferReader, CircularBufferWriter};
use nix::sys::stat::Mode;
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::ptr::slice_from_raw_parts_mut;

pub struct SharedMemoryBufferWriter {
    write_status: PtrStatus,
    backend: SharedMemoryBuffer,
    _file_lock: AdvisoryFileLock,
}

impl SharedMemoryBufferWriter {
    pub fn open(name: impl Into<String>) -> io::Result<Self> {
        let name = name.into();

        let _file_lock = AdvisoryFileLock::try_lock(
            SharedMemoryBufferWriter::lock_path(name.as_str()),
            &OpenOptions::new().create(true).read(true).write(true),
        )?;

        let backend = SharedMemoryBuffer::open(name)?;

        Ok(Self {
            write_status: backend.write_status(),
            backend,
            _file_lock,
        })
    }

    pub fn create(
        name: impl Into<String>,
        size: u64,
        alignment_pow2: u8,
        permissions: Mode,
    ) -> io::Result<Self> {
        let name = name.into();

        let _file_lock = AdvisoryFileLock::try_lock(
            SharedMemoryBufferReader::lock_path(name.as_str()),
            &OpenOptions::new().create(true).read(true).write(true),
        )?;

        let _reader_file_lock = AdvisoryFileLock::try_lock(
            SharedMemoryBufferWriter::lock_path(name.as_str()),
            &OpenOptions::new().create(true).read(true).write(true),
        )?;

        let backend = SharedMemoryBuffer::create(name, size, alignment_pow2, permissions)?;

        Ok(Self {
            write_status: backend.write_status(),
            backend,
            _file_lock,
        })
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.backend.alignment_pow2()
    }

    pub(super) fn lock_path(name: &str) -> PathBuf {
        format!("/tmp/{}_writer.lock", name).into()
    }
}

impl SharedMemoryBufferWriter {
    pub fn advance_write_pointer(&mut self, bytes: usize) -> Result<(), SharedMemoryBufferError> {
        if !ebutils::check_alignment_pow2(bytes, self.backend.alignment_pow2()) {
            return Err(SharedMemoryBufferError::AlignmentViolation);
        }

        if self.writable_length() < bytes {
            return Err(SharedMemoryBufferError::OutOfBounds);
        }

        self.write_status = self.write_status.wrapped_offset(bytes, self.backend.size());
        self.backend.set_write_status(self.write_status);

        Ok(())
    }

    pub fn writable_region(&mut self) -> (&mut [u8], &mut [u8]) {
        let read_status = self.backend.read_status();
        let same_wrap = read_status.wrap_flag() == self.write_status.wrap_flag();
        let buffer_address = self.backend.buffer_address_mut();

        if same_wrap {
            // Primary region: from write_ptr to end
            // Secondary region: from start to read_ptr

            let primary_region_address =
                unsafe { buffer_address.add(self.write_status.address() as usize) };
            let primary_region_length = self.backend.size() - self.write_status.address() as usize;
            let primary_region = unsafe {
                &mut *slice_from_raw_parts_mut(primary_region_address, primary_region_length)
            };

            let secondary_region_address = buffer_address;
            let secondary_region_length = read_status.address() as usize;
            let secondary_region = unsafe {
                &mut *slice_from_raw_parts_mut(secondary_region_address, secondary_region_length)
            };

            (primary_region, secondary_region)
        } else {
            // Primary region: from write_ptr to read_ptr
            // Secondary region: empty

            let primary_region_address =
                unsafe { buffer_address.add(self.write_status.address() as usize) };
            let primary_region_length =
                read_status.address() as usize - self.write_status.address() as usize;
            let primary_region = unsafe {
                &mut *slice_from_raw_parts_mut(primary_region_address, primary_region_length)
            };

            let secondary_region = &mut [];

            (primary_region, secondary_region)
        }
    }

    pub fn writable_length(&mut self) -> usize {
        let readable_region = self.writable_region();
        readable_region.0.len() + readable_region.1.len()
    }
}

impl CircularBufferWriter for SharedMemoryBufferWriter {
    type AdvanceResult = Result<(), SharedMemoryBufferError>;
    type WriteableRegionResult<'a> = (&'a mut [u8], &'a mut [u8]);

    fn advance_write_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        SharedMemoryBufferWriter::advance_write_pointer(self, bytes)
    }
    fn writable_region(&mut self) -> Self::WriteableRegionResult<'_> {
        SharedMemoryBufferWriter::writable_region(self)
    }
}

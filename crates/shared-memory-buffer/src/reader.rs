use crate::backend::{SharedMemoryBuffer, SharedMemoryBufferError};
use crate::header::PtrStatus;
use crate::posix_advisory_file_lock::AdvisoryFileLock;
use crate::writer::SharedMemoryBufferWriter;
use nix::sys::stat::Mode;
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::ptr::slice_from_raw_parts;

pub struct SharedMemoryBufferReader {
    read_status: PtrStatus,
    backend: SharedMemoryBuffer,
    _file_lock: AdvisoryFileLock,
}

impl SharedMemoryBufferReader {
    pub fn open(name: impl Into<String>) -> io::Result<Self> {
        let name = name.into();

        let _file_lock = AdvisoryFileLock::try_lock(
            SharedMemoryBufferReader::lock_path(name.as_str()),
            &OpenOptions::new().create(true).read(true).write(true),
        )?;

        let backend = SharedMemoryBuffer::open(name)?;

        Ok(Self {
            read_status: backend.read_status(),
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

        let _writer_file_lock = AdvisoryFileLock::try_lock(
            SharedMemoryBufferWriter::lock_path(name.as_str()),
            &OpenOptions::new().create(true).read(true).write(true),
        )?;

        let _file_lock = AdvisoryFileLock::try_lock(
            SharedMemoryBufferReader::lock_path(name.as_str()),
            &OpenOptions::new().create(true).read(true).write(true),
        )?;

        let backend = SharedMemoryBuffer::create(name, size, alignment_pow2, permissions)?;

        Ok(Self {
            read_status: backend.read_status(),
            backend,
            _file_lock,
        })
    }

    pub(super) fn lock_path(name: &str) -> PathBuf {
        format!("/tmp/{}_reader.lock", name).into()
    }
}

impl SharedMemoryBufferReader {
    pub fn advance_read_pointer(&mut self, bytes: usize) -> Result<(), SharedMemoryBufferError> {
        if !ebutils::check_alignment_pow2(bytes, self.backend.alignment_pow2()) {
            return Err(SharedMemoryBufferError::AlignmentViolation);
        }

        if self.readable_length() < bytes {
            return Err(SharedMemoryBufferError::OutOfBounds);
        }

        self.read_status = self.read_status.wrapped_offset(bytes, self.backend.size());
        self.backend.set_read_status(self.read_status);

        Ok(())
    }

    pub fn readable_region(&self) -> (&[u8], &[u8]) {
        let write_status = self.backend.write_status();
        let same_wrap = write_status.wrap_flag() == self.read_status.wrap_flag();
        let buffer_address = self.backend.buffer_address();

        if same_wrap {
            // Primary region: from read_ptr to write_ptr
            // Secondary region: empty

            let primary_region_address =
                unsafe { buffer_address.add(self.read_status.address() as usize) };
            let primary_region_length =
                write_status.address() as usize - self.read_status.address() as usize;
            let primary_region =
                unsafe { &*slice_from_raw_parts(primary_region_address, primary_region_length) };

            let secondary_region = &[];

            (primary_region, secondary_region)
        } else {
            // Primary region: from read_ptr to end
            // Secondary region: from start to write_ptr

            let primary_region_address =
                unsafe { buffer_address.add(self.read_status.address() as usize) };
            let primary_region_length = self.backend.size() - self.read_status.address() as usize;
            let primary_region =
                unsafe { &*slice_from_raw_parts(primary_region_address, primary_region_length) };

            let secondary_region_address = buffer_address;
            let secondary_region_length = write_status.address() as usize;
            let secondary_region = unsafe {
                &*slice_from_raw_parts(secondary_region_address, secondary_region_length)
            };

            (primary_region, secondary_region)
        }
    }

    pub fn readable_length(&self) -> usize {
        let readable_region = self.readable_region();
        readable_region.0.len() + readable_region.1.len()
    }
}

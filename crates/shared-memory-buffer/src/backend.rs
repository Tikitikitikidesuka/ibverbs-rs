use crate::header::{Header, PtrStatus};
use crate::posix_shared_memory::{AccessMode, MappedSharedMemory, SharedMemory};
use nix::sys::stat::Mode;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharedMemoryBufferError {
    #[error("Address not aligned")]
    AlignmentViolation,
    #[error("Out of bounds")]
    OutOfBounds,
}

pub struct SharedMemoryBuffer {
    name: String,
    shared_memory: MappedSharedMemory,
    alignment_pow2: u8,
    buffer_address: *mut u8,
    buffer_size: usize,
}

impl SharedMemoryBuffer {
    pub fn open(name: impl Into<String>) -> io::Result<Self> {
        let name = name.into();

        let shared_memory = unsafe {
            SharedMemory::open(name.as_str(), AccessMode::ReadWrite)?.map(AccessMode::ReadWrite)?
        };

        println!("Shared memory len: {}", shared_memory.len());

        if shared_memory.len() < size_of::<Header>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "shared memory size is not big enough for the buffer",
            ));
        }

        let alignment_pow2 =
            unsafe { (*(shared_memory.as_ptr() as *const Header)).alignment_pow2 } as u8;
        let buffer_address = ebutils::align_up_pow2(
            shared_memory.as_ptr() as usize + size_of::<Header>(),
            alignment_pow2,
        ) as *mut u8;
        let buffer_size = unsafe { (*(shared_memory.as_ptr() as *const Header)).size } as usize;

        Ok(Self {
            name,
            shared_memory,
            alignment_pow2,
            buffer_address,
            buffer_size,
        })
    }

    pub fn create(
        name: impl Into<String>,
        size: u64,
        alignment_pow2: u8,
        permissions: Mode,
    ) -> io::Result<Self> {
        let name = name.into();

        if alignment_pow2 < 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "buffer has a minimum alignment of 2 bytes",
            ));
        }

        let shared_memory = unsafe {
            SharedMemory::create(
                name.as_str(),
                size as usize + size_of::<Header>(),
                AccessMode::ReadWrite,
                permissions,
            )?
            .map(AccessMode::ReadWrite)?
        };

        unsafe {
            *(shared_memory.as_mut_ptr() as *mut Header) = Header {
                write_status: PtrStatus::zero(),
                read_status: PtrStatus::zero(),
                size,
                alignment_pow2: alignment_pow2 as u64,
                id: 0,
            }
        };

        let buffer_address = ebutils::align_up_pow2(
            shared_memory.as_ptr() as usize + size_of::<Header>(),
            alignment_pow2,
        ) as *mut u8;
        let buffer_size = unsafe { (*(shared_memory.as_ptr() as *const Header)).size } as usize;

        Ok(Self {
            name,
            shared_memory,
            alignment_pow2,
            buffer_address,
            buffer_size,
        })
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn size(&self) -> usize {
        self.buffer_size
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.alignment_pow2
    }

    pub fn write_status(&self) -> PtrStatus {
        unsafe { (*(self.shared_memory.as_ptr() as *const Header)).write_status }
    }

    pub fn set_write_status(&mut self, write_status: PtrStatus) {
        unsafe {
            (*(self.shared_memory.as_mut_ptr() as *mut Header)).write_status = write_status;
        }
    }

    pub fn read_status(&self) -> PtrStatus {
        unsafe { (*(self.shared_memory.as_ptr() as *const Header)).read_status }
    }

    pub fn set_read_status(&mut self, read_status: PtrStatus) {
        unsafe {
            (*(self.shared_memory.as_mut_ptr() as *mut Header)).read_status = read_status;
        }
    }

    pub fn buffer_address(&self) -> *const u8 {
        self.buffer_address
    }

    pub fn buffer_address_mut(&mut self) -> *mut u8 {
        self.buffer_address
    }
}

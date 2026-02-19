use std::{
    fmt::Debug,
    fs::File,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::Path,
};

use bytemuck::{cast_slice_mut, try_cast_slice};
use std::io::Result as IoResult;

use crate::{MdfRecord, header::Unknown};

/// This struct represents a sequence of MDF records backed by some `Store`.
///
/// You can use it to access MDF records using [`Self::mdf_record_iter`].
///
/// It can be cerated, either by copying over a slice (to ensure `u32` alignment), wrapping a aligned slice,
/// or from a file, copying it into memory or using mmap if the corresponding feature is enabled.
///
/// # Example
/// ```rust
/// # use master_data_file::MdfFile;
/// let mdf_file = MdfFile::from_data(include_bytes!("../test.mdf"));
/// for record in mdf_file.mdf_record_iter() {
///     if let Ok(record) = record.try_into_single_event() {
///         for fragment in record.fragments() {
///             // do something with the fragment
///         }
///     }
/// }
/// ```
pub struct MdfFile<Store: AsRef<[u32]> = Box<[u32]>> {
    data: Store,
}

impl<Store: AsRef<[u32]>> MdfFile<Store> {
    /// Returns an iterator over all the MDF records in this file.
    pub fn mdf_record_iter(&self) -> MdfRecordIterator<'_> {
        MdfRecordIterator {
            data: self.data.as_ref(),
        }
    }

    /// Returns the raw data of this file.
    pub fn data(&self) -> &[u32] {
        self.data.as_ref()
    }

    /// Retrieves the underlying store.
    pub fn into_inner(self) -> Store {
        self.data
    }
}

impl<'a, Store: AsRef<[u32]>> IntoIterator for &'a MdfFile<Store> {
    type Item = &'a MdfRecord;

    type IntoIter = MdfRecordIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.mdf_record_iter()
    }
}

impl MdfFile<Box<[u32]>> {
    /// Creates a new store by copying over data to ensure alignment.
    ///
    /// There is also a version without copying, if the data is already aligned: [`Self::from_aligned_slice`].
    ///
    /// Data must contain valid mdf records.
    /// Data will be copied to ensure alignment.
    pub fn from_data(data: &[u8]) -> Self {
        let mut boxed = vec![0u32; data.len().div_ceil(size_of::<u32>())].into_boxed_slice();
        cast_slice_mut(&mut boxed)[..data.len()].copy_from_slice(data);
        Self { data: boxed }
    }

    /// Reads an MDF file into memory (completely).
    pub fn read_file(file: impl AsRef<Path>) -> IoResult<Self> {
        let mut file: File = File::open(file)?;
        let size = file.metadata()?.size();
        let size = usize::try_from(size).map_err(io::Error::other)?;
        let mut data = vec![0u32; size / size_of::<u32>()].into_boxed_slice();
        file.read_exact(&mut cast_slice_mut(&mut data)[..size])?;
        Ok(Self { data })
    }
}

impl<'a> MdfFile<&'a [u32]> {
    /// Creates an MDF file from an aligned slice, without any copying.
    ///
    /// Returns `None` if the slice is not 32 bit aligned or has size not multiple of 32 bit.
    pub fn from_aligned_slice(data: &'a [u8]) -> Option<Self> {
        try_cast_slice(data).map(|data| Self { data }).ok()
    }
}

#[cfg(feature = "mmap")]
pub mod mmap {
    use super::*;

    use memmap2::Mmap;

    /// Wrapper struct for a `u32` aligned memory mapped region.
    ///
    /// Expects memory mapped files to be page aligned, and pages to be larger than `u32` 😉.
    pub struct MemMap(Mmap);

    impl AsRef<[u32]> for MemMap {
        fn as_ref(&self) -> &[u32] {
            bytemuck::try_cast_slice(self.0.as_ref()).expect("alignment matches, length compatible")
        }
    }

    impl MdfFile<MemMap> {
        /// Creates a new `MdfFile` by memory mapping a file at the given path.
        ///
        /// The advantage of this over using [`MdfFile::read_file`] is that the file
        /// must not be read into memory at once but only as needed.
        pub fn mmap_file(file: impl AsRef<Path>) -> IoResult<Self> {
            let file = File::open(file)?;
            let map = unsafe { Mmap::map(&file) }?;
            #[cfg(unix)]
            {
                map.advise(memmap2::Advice::Sequential)?;
            }
            Ok(MdfFile { data: MemMap(map) })
        }
    }
}

/// An iterator over MDF records in an MDF file.
pub struct MdfRecordIterator<'a> {
    data: &'a [u32],
}

impl<'a> Iterator for MdfRecordIterator<'a> {
    type Item = &'a MdfRecord<Unknown>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }

        // todo make fallible
        let (record, rest) = MdfRecord::from_data(self.data).expect("valid mdf data");
        self.data = rest;

        Some(record)
    }
}

impl<D: AsRef<[u32]>> Debug for MdfFile<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.mdf_record_iter().map(|r| {
                r.try_into_single_event()
                    .map(|r| r as &dyn Debug)
                    .unwrap_or(r)
            }))
            .finish()
    }
}

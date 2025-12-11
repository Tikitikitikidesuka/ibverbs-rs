use std::{borrow::Borrow, ops::Deref};

use crate::{MultiEventPacket, builder::MultiEventPacketBuilder};

/// This struct represents an owned [`MultiEventPacket`].
///
/// Its relationship to [`MultiEventPacket`] is as [`String`] to [`str`].
///
/// An owned MEP can be constructed using the [`MultiEventPacketBuilder`].
#[derive(Clone)]
pub struct MultiEventPacketOwned<Data: AsRef<[u32]> = Box<[u32]>> {
    data: Data, // assures alignment of u32
}

impl<D: AsRef<[u32]>> Deref for MultiEventPacketOwned<D> {
    type Target = MultiEventPacket;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<D: AsRef<[u32]>> AsRef<MultiEventPacket> for MultiEventPacketOwned<D> {
    fn as_ref(&self) -> &MultiEventPacket {
        // MultiEventPacket must be guaranteed to be correct already. Since it can only
        // be built by the builder, it is supposed to be guaranteed.
        // todo enforce this...
        unsafe { MultiEventPacket::unchecked_from_raw_bytes(self.data.as_ref()) }
    }
}

impl<D: AsRef<[u32]>> Borrow<MultiEventPacket> for MultiEventPacketOwned<D> {
    fn borrow(&self) -> &MultiEventPacket {
        self
    }
}

impl MultiEventPacketOwned {
    /// Returns a new builder instance for building a owned MEP.
    pub fn builder<'a>() -> MultiEventPacketBuilder<'a> {
        MultiEventPacketBuilder::new()
    }
}

impl<D: AsRef<[u32]>> MultiEventPacketOwned<D> {
    /// Creates a new owned MEP from a boxed slice of `u32`.
    /// # Safety
    /// Data needs to be a valid [`MultiEventPacket`].
    pub unsafe fn from_data(data: D) -> Self {
        Self { data }
    }
}

#[cfg(feature = "mmap")]
pub mod mmap {
    use std::{fs::File, path::Path};

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

    impl MultiEventPacketOwned<MemMap> {
        /// Creates a new `MdfFile` by memory mapping a file at the given path.
        ///
        /// The advantage of this over using [`MdfFile::read_file`] is that the file
        /// must not be read into memory at once but only as needed.
        pub fn mmap_file(file: impl AsRef<Path>) -> std::io::Result<Self> {
            let file = File::open(file)?;
            let map = unsafe { Mmap::map(&file) }?;
            #[cfg(unix)]
            {
                map.advise(memmap2::Advice::Sequential);
            }

            // todo enforce valid mep...
            Ok(unsafe { Self::from_data(MemMap(map)) })
        }
    }
}

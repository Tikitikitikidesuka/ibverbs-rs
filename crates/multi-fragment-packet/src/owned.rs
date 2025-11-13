use std::{borrow::Borrow, ops::Deref};

use crate::{
    MultiFragmentPacket, MultiFragmentPacketBuilder, MultiFragmentPacketFromRawBytesError,
};

/// This struct represents an owned [`MultiFragmentPacket`].
///
/// Its relationship to [`MultiFragmentPacket`] is as [`String`] to [`str`].
///
/// An owned MFP can either be constructed from a `Vec<u8>` or using the [`Self::builder`] method.
#[derive(Clone)]
pub struct MultiFragmentPacketOwned {
    /// SAFETY INVARIANT: contains a valid MFP.
    data: Vec<u8>,
}

impl MultiFragmentPacketOwned {
    /// This function tries to create a new owned MFP from raw bytes.
    ///
    /// It has the same preconditions as [`MultiFragmentPacket::from_raw_bytes`].
    pub fn from_data(data: Vec<u8>) -> Result<Self, MultiFragmentPacketFromRawBytesError> {
        let _test = MultiFragmentPacket::from_raw_bytes(&data)?;
        Ok(Self { data })
    }

    /// Creates an owned MFP from raw bytes without checking anything.
    ///
    /// # Safety
    /// You need to ensure that `data` contains a valid MFP.
    /// For the requirements, see [`MultiFragmentPacket::from_raw_bytes`].
    pub unsafe fn from_data_unchecked(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Returns a typed builder to construct an MFP.
    ///
    /// The following fields are required:
    /// - `with_event_id(EventId)`
    /// - `with_source_id(SourceId)`
    /// - `with_align_log(u8)`
    /// - `with_fragment_version(u8)`
    ///
    /// To add fragments, use:
    /// - `add_fragment(FragmentType, impl Into<Vec<u8>>)`
    /// - `add_fragments(impl IntoIterator over (FragmentType, Into<Vec<u8>>))`
    ///
    /// This adds a fragment with the given type and payload.
    /// Note that [`ebutils::OdinPayload`] implements `Into<Vec<u8>>`.
    ///
    /// The following fields are optional:
    /// - `with_magic(u16)` (default: [`MultiFragmentPacket::VALID_MAGIC`])
    pub fn builder() -> MultiFragmentPacketBuilder {
        MultiFragmentPacketBuilder::default()
    }
}

impl AsRef<MultiFragmentPacket> for MultiFragmentPacketOwned {
    fn as_ref(&self) -> &MultiFragmentPacket {
        // SAFETY: by invariant of this type a valid MFP.
        unsafe { MultiFragmentPacket::unchecked_ref_from_raw_bytes(self.data.as_slice()) }
    }
}

impl Deref for MultiFragmentPacketOwned {
    type Target = MultiFragmentPacket;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl ToOwned for MultiFragmentPacket {
    type Owned = MultiFragmentPacketOwned;

    fn to_owned(&self) -> Self::Owned {
        // SAFETY: Already is a valid MFP.
        unsafe { Self::Owned::from_data_unchecked(self.raw_packet_data().to_vec()) }
    }
}

impl Borrow<MultiFragmentPacket> for MultiFragmentPacketOwned {
    fn borrow(&self) -> &MultiFragmentPacket {
        self
    }
}

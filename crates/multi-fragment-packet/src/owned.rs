use std::{borrow::Borrow, ops::Deref};

use crate::{
    MultiFragmentPacket, MultiFragmentPacketBuilder, MultiFragmentPacketFromRawBytesError,
};

/// This struct represents an owned [`MultiFragmentPacket`].
///
/// Its relationship to [`MultiFragmentPacket`] is as [`String`] to [`str`].
///
/// An owned MFP can either be constructed from a `Vec<u8>` or using the [`Self::builder`] method.
pub struct MultiFragmentPacketOwned {
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
    /// Note that [`ebutil::OdinPayload`] implements `Into<Vec<u8>>`.
    /// todo
    pub fn builder() -> MultiFragmentPacketBuilder {
        MultiFragmentPacketBuilder::default()
    }
}

impl AsRef<MultiFragmentPacket> for MultiFragmentPacketOwned {
    fn as_ref(&self) -> &MultiFragmentPacket {
        // MultiFragmentPacket must be guaranteed to be correct already. Since it can only
        // be built by the builder, it is supposed to be guaranteed.
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
        unsafe { Self::Owned::from_data(self.raw_packet_data().to_vec()) }
    }
}

impl Borrow<MultiFragmentPacket> for MultiFragmentPacketOwned {
    fn borrow(&self) -> &MultiFragmentPacket {
        self
    }
}

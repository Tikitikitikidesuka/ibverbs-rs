use std::{borrow::Borrow, ops::Deref};

use crate::{MultiEventPacket, builder::MultiEventPacketBuilder};

/// This struct represents an owned [`MultiEventPacket`].
///
/// Its relationship to [`MultiEventPacket`] is as [`String`] to [`str`].
///
/// An owned MEP can be constructed using the [`MultiEventPacketBuilder`].
#[derive(Clone)]
pub struct MultiEventPacketOwned {
    data: Box<[u32]>, // assures alignement of u32
}

impl Deref for MultiEventPacketOwned {
    type Target = MultiEventPacket;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsRef<MultiEventPacket> for MultiEventPacketOwned {
    fn as_ref(&self) -> &MultiEventPacket {
        // MultiEventPacket must be guaranteed to be correct already. Since it can only
        // be built by the builder, it is supposed to be guaranteed.
        unsafe { MultiEventPacket::unchecked_ref_from_raw_bytes(&self.data) }
    }
}

impl Borrow<MultiEventPacket> for MultiEventPacketOwned {
    fn borrow(&self) -> &MultiEventPacket {
        self
    }
}

impl MultiEventPacketOwned {
    /// Returns a new builder instance for building a owned MEP.
    pub fn builder<'a>() -> MultiEventPacketBuilder<'a> {
        MultiEventPacketBuilder::new()
    }

    /// Creates a new owned MEP from a boxed slice of `u32`.
    /// # Safety
    /// Data needs to be a valid [`MultiEventPacket`].
    pub unsafe fn from_data(data: Box<[u32]>) -> Self {
        Self { data }
    }
}

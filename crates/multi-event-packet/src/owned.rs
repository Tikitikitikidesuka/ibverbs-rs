use std::{borrow::Borrow, ops::Deref};

use crate::{MultiEventPacket, builder::MultiEventPacketBuilder};

/// Container type owning a [`MultiEventPacket`].
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
    pub fn builder<'a>() -> MultiEventPacketBuilder<'a> {
        MultiEventPacketBuilder::new()
    }

    /// # Safety
    /// Data needs to be a valid [`MultiEventPacket`].
    pub unsafe fn from_data(data: Box<[u32]>) -> Self {
        Self { data }
    }
}

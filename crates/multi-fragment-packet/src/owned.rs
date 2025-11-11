use std::{borrow::Borrow, ops::Deref};

use crate::{MultiFragmentPacket, MultiFragmentPacketBuilder};

pub struct MultiFragmentPacketOwned {
    data: Vec<u8>,
}

impl MultiFragmentPacketOwned {
    /// # Safety
    /// Vec needs to contain a valid [`MultiFragmentPacket`].
    #[must_use]
    pub unsafe fn from_data(data: Vec<u8>) -> Self {
        Self { data }
    }

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

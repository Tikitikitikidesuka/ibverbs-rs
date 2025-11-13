use std::{fmt::Display, ops::Range};

use bytemuck::{Pod, Zeroable};
use derive_more::UpperHex;
use strum::{FromRepr, IntoStaticStr};

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, UpperHex, Zeroable, Pod)]
pub struct SourceId(pub u16);

impl Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.sub_detector() {
            Ok(sub) => write!(f, "{sub:?}-{:#06X} ({:#X})", self.sub_part(), self.0),
            Err(_id) => write!(f, "Unknown-{:#06X} ({:#X})", self.sub_part(), self.0),
        }
    }
}

impl SourceId {
    pub const BITS: u32 = u16::BITS;

    pub const fn new(detector: SubDetector, sub_id: u16) -> Self {
        detector.to_source_id(sub_id)
    }

    pub fn new_odin(odin_number: u16) -> Self {
        SubDetector::Odin.to_source_id(odin_number)
    }

    pub fn sub_detector(self) -> Result<SubDetector, u8> {
        SubDetector::from_source_id(self)
    }

    pub fn sub_part(self) -> u16 {
        self.0 & (1 << ((SourceId::BITS - SubDetector::BITS) - 1))
    }

    pub fn is_odin(self) -> bool {
        self.sub_detector().is_ok_and(|s| s == SubDetector::Odin)
    }

    /// Returns the odin number of this source id, if it is a odin source id.
    pub fn odin_number(self) -> Option<u16> {
        self.is_odin().then_some(self.sub_part())
    }
}

#[repr(u8)]
#[derive(FromRepr, Clone, Copy, Debug, IntoStaticStr, PartialEq, Eq)]
pub enum SubDetector {
    Odin = 0,
    VeloA = 2,
    VeloC,
    Rich1,
    UtA,
    UtC,
    ScifiA,
    ScifiC,
    Rich2,
    Plume,
    Ecal,
    Hcal,
    MuonA,
    MuonC,
    Tdet,
}

impl SubDetector {
    const BITS: u32 = 5;
    pub const fn to_source_id(self, sub_id: u16) -> SourceId {
        assert!(sub_id < 1 << (SourceId::BITS - Self::BITS));
        SourceId((self as u8 as u16) << (SourceId::BITS - Self::BITS) | sub_id)
    }

    pub fn from_source_id(id: SourceId) -> Result<Self, u8> {
        let bin = (id.0 >> (SourceId::BITS - Self::BITS)) as u8;
        Self::from_repr(bin).ok_or(bin)
    }

    pub fn source_id_range(self) -> Range<SourceId> {
        let id = self.to_source_id(0);
        id..SourceId(id.0 + (1 << (SourceId::BITS - SubDetector::BITS)))
    }
}

#[cfg(test)]
mod test {
    use crate::source_id::SubDetector;

    #[test]
    fn test_sub_id() {
        assert_eq!(SubDetector::UtC.to_source_id(0x3).0, 0x3003);
    }
}

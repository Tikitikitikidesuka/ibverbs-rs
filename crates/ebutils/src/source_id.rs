use std::{fmt::Display, ops::Range};

use bytemuck::{Pod, Zeroable};
use derive_more::UpperHex;
use strum::{FromRepr, IntoStaticStr};

/// This struct represents a source id for data fragments.
///
/// It is just a wrapper around a `u16`, but provides methods to extract the sub-detector and sub-part information.
///
/// Source IDs are defined here <https://edms.cern.ch/ui/file/2100937/5/edms_2100937_raw_data_format_run3.pdf#subsection.1.4>.
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

    /// Creates a new source id from the given sub-detector and sup part id within that sub-detector.
    pub const fn new(detector: SubDetector, sub_id: u16) -> Self {
        detector.to_source_id(sub_id)
    }

    /// Creates a new source id for an Odin fragment, which has its sub detector part equal to 0.
    /// The
    pub fn new_odin(odin_number: u16) -> Self {
        SubDetector::Odin.to_source_id(odin_number)
    }

    /// Tries to parse the sub_detector from this source id.
    ///
    /// Returns an error containing the raw sub-detector id if the sub-detector id is none of the known ones.
    pub fn sub_detector(self) -> Result<SubDetector, u8> {
        SubDetector::from_source_id(self)
    }

    /// Returns the 11 sub detector specific bits of this source id.
    pub fn sub_part(self) -> u16 {
        self.0 & ((1 << (SourceId::BITS - SubDetector::BITS)) - 1)
    }

    /// Returns true if this is the source id for odin fragments.
    pub fn is_odin(self) -> bool {
        self.sub_detector().is_ok_and(|s| s == SubDetector::Odin)
    }

    /// Returns the *odin number* of this source id, if it is a odin source id.
    pub fn odin_number(self) -> Option<u16> {
        self.is_odin().then_some(self.sub_part())
    }
}

/// Sub-detectors known in LHCb.
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

    /// Creates a source id from the sub-detector with the given sub-part id.
    pub const fn to_source_id(self, sub_id: u16) -> SourceId {
        assert!(sub_id < 1 << (SourceId::BITS - Self::BITS));
        SourceId((self as u8 as u16) << (SourceId::BITS - Self::BITS) | sub_id)
    }

    /// Tries to parse a sub-detector from the given source id.
    ///
    /// If the given source id does not correspond to a known sub-detector, returns an error containing the raw sub-detector id.
    pub fn from_source_id(id: SourceId) -> Result<Self, u8> {
        let bin = (id.0 >> (SourceId::BITS - Self::BITS)) as u8;
        Self::from_repr(bin).ok_or(bin)
    }

    /// Returns the range of source ids that belong to this sub-detector.
    ///
    /// Useful for querying all fragments from a specific sub-detector from an mep.
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
        assert_eq!(SubDetector::Tdet.to_source_id(0x3).0, 0x7803);
    }
}

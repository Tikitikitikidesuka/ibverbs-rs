use strum::{AsRefStr, FromRepr, IntoStaticStr};

use crate::SourceId;

#[repr(u8)]
#[derive(FromRepr, Clone, Copy, Debug, IntoStaticStr)]
pub enum SubDetectors {
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

impl SubDetectors {
    const BITS: u32 = 5;
    pub fn to_source_id(self, sub_id: SourceId) -> SourceId {
        assert!(sub_id < 1 << (SourceId::BITS - Self::BITS));
        (self as u8 as SourceId) << (SourceId::BITS - Self::BITS) | sub_id
    }

    pub fn from_source_id(id: SourceId) -> Option<Self> {
        Self::from_repr((id >> (SourceId::BITS - Self::BITS)) as u8)
    }
}

#[cfg(test)]
mod test {
    use crate::sub_detector::SubDetectors;

    #[test]
    fn test_sub_id() {
        assert_eq!(SubDetectors::UtC.to_source_id(0x3), 0x3003);
    }
}

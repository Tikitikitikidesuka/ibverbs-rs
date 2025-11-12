use strum::{FromRepr, IntoStaticStr, VariantArray};

/// Routing bits that may be set in MDF single event specific header event mask, from <https://edms.cern.ch/ui/file/1146861/1.4/rb_note.pdf>.
///
/// The routing bits used to define data streams, together with the name of the are as follows:
/// All the above routing bits have been present since the start of physics data taking in Run II. Routing
/// bit 91 has been used for the NOBIAS stream in the 2015 EM campaign and was subsequently turned
/// off; it may be recycled for other purposes in the future
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr, IntoStaticStr, VariantArray)]
#[non_exhaustive]
pub enum RoutingBit {
    Lumi = 33,
    BeamGas = 35,
    VeloCloseMonitor = 40,
    MuonAlignment = 42,
    HLT1Physics = 46,
    TrackerAlignment = 53,
    RICHMirrorAlignment = 54,
    ODINNoBias = 55,
    FullStream = 87,
    TurboStream = 88,
    ParkedStream = 89,
    /// TURCAL
    CalibrationStream = 90,
    /// 2015 EM NoBias
    NoBias = 91,
    // 92-96 Future Streams
}

impl RoutingBit {
    pub const ROUTING_MASK: u128 = {
        let mut mask = 0u128;
        let mut idx = 0;
        while idx < RoutingBit::VARIANTS.len() {
            let bit = RoutingBit::VARIANTS[idx];
            mask |= 1u128 << (bit as u8);
            idx += 1;
        }
        mask
    };
}

use bytemuck::{Pod, Zeroable};
use strum::FromRepr;

use crate::header::{SpecificHeaderType, SpecificHeaderTypeAndSize};

/// Multi purpouse type for MDF records, as defined [here](https://edms.cern.ch/ui/file/784588/2/Online_Raw_Data_Format.pdf#page=7).
///
/// This type currently only exists "as-is", without any special functionality.
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
#[repr(C, align(4))]
pub struct MultiPurpose {}
impl super::internal::Sealed for MultiPurpose {}
/// # Safety
/// Size is 0, multiple of 4.
unsafe impl SpecificHeaderType for MultiPurpose {
    const HEADER_TYPE: u8 = 4;
    fn header_type_and_size() -> SpecificHeaderTypeAndSize {
        SpecificHeaderTypeAndSize::from_type_and_size(Self::HEADER_TYPE, 0)
    }
}

/// Type of a multi purpose mdf fragment.
#[repr(u8)]
#[derive(Copy, Clone, FromRepr)]
pub enum MultiPurposeType {
    /// Sequences of banks produced by TELL1 boards as described in [^1].
    /// [^1]: O.Callot et al., Raw Data Format. EDMS note 565851.
    BodyTypeBanks = 1,
    /// Full MEP records including the transport format as defined in [^3]. This data type is used to process time alignment data [^2].
    /// [^2]: O.Callot, Processing Time-Alignment Events. EDMS note 779819.
    /// [^3]: B.Jost, N.Neufeld, Raw-data transport format. EDMS note 499933
    BodyTypeMEP = 2,
}

impl MultiPurposeType {
    /// Value of a variant as stored in hte header.
    pub const fn value(&self) -> u8 {
        *self as u8
    }
}

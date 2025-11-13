use bytemuck::{Pod, Zeroable};
use ebutils::OdinPayload;

use crate::{
    header::{MdfHeader, SpecificHeaderType, SpecificHeaderTypeAndSize},
    rounting_bits::RoutingBit,
};

/// A type of MDF record that contains multiple fragments for a single event.
///
/// It must contain exactly one odin fragment.
#[repr(C, packed(4))]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct SingleEvent {
    /// This mask contains various bit flags, including the [`RoutingBit`]s.
    pub event_mask: u128,
    pub run_number: u32,
    pub orbit_count: u32,
    pub bunch_identifier: u32,
}

impl super::internal::Sealed for SingleEvent {}
/// ## Safety
/// Size is 28, multiple of 4.
unsafe impl SpecificHeaderType for SingleEvent {
    #[allow(clippy::cast_possible_truncation)]
    const HEADER_TYPE: u8 = (size_of::<SingleEvent>() / size_of::<u32>()) as u8;

    fn header_type_and_size() -> SpecificHeaderTypeAndSize {
        SpecificHeaderTypeAndSize::from_type_and_size(Self::HEADER_TYPE, Self::HEADER_SIZE_U32)
    }
}
impl SingleEvent {
    /// Size of a single event specific header;
    pub const HEADER_SIZE_U32: u8 = 7;

    /// Returns the routing bit set for this MDF record.
    ///
    /// The routing bit decides where the record gets routed to in the data mover.
    /// Only one routing bit may be set at a time.
    pub fn get_routing_bit(&self) -> Option<RoutingBit> {
        let rounting_bits = self.event_mask & RoutingBit::ROUTING_MASK;
        if rounting_bits != 0 {
            if !rounting_bits.is_power_of_two() {
                panic!(
                    "Multiple routing bits set in MDF header event mask: {:0b}",
                    rounting_bits
                );
            }
            RoutingBit::from_repr(
                rounting_bits
                    .ilog2()
                    .try_into()
                    .expect("only 128 < 255 bit positions"),
            )
        } else {
            None
        }
    }
}

impl MdfHeader<SingleEvent> {
    /// Creates a new simple MDF header of type `SingleEvent`.
    ///
    /// Does not use any checksum or compression, nor does it have any `event_mask` flags set.
    pub fn new_simple(payload_size: usize, odin: OdinPayload) -> Self {
        let length_32 =
            u32::try_from(payload_size + size_of::<Self>()).expect("payload size fits in u32");

        MdfHeader {
            lengths: [length_32; 3],
            checksum: 0,
            compression: 0,
            header_type_and_size: SingleEvent::header_type_and_size(),
            data_type: 0,
            _spare: 0,
            specific_header: SingleEvent {
                event_mask: 0,
                run_number: odin.run_number(),
                orbit_count: odin.orbit_id(),
                bunch_identifier: odin.bunch_id() as _,
            },
        }
    }

    /// Creates new MDF header of type `SingleEvent` with a routing bit set.
    ///
    /// Does not use any checksum or compression.
    pub fn new_with_routing_bit(
        payload_size: usize,
        odin: OdinPayload,
        routing_bit: RoutingBit,
    ) -> Self {
        let length_32 =
            u32::try_from(payload_size + size_of::<Self>()).expect("payload size fits in u32");

        MdfHeader {
            lengths: [length_32; 3],
            checksum: 0,
            compression: 0,
            header_type_and_size: SingleEvent::header_type_and_size(),
            data_type: 0,
            _spare: 0,
            specific_header: SingleEvent {
                event_mask: 1 << routing_bit as u128,
                run_number: odin.run_number(),
                orbit_count: odin.orbit_id(),
                bunch_identifier: odin.bunch_id() as _,
            },
        }
    }
}

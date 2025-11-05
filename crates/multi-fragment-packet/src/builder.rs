use typed_builder::TypedBuilder;

use crate::{EventId, MultiFragmentPacket, MultiFragmentPacketHeader, MultiFragmentPacketRef};

#[derive(TypedBuilder)]
#[builder(build_method(into = crate::MultiFragmentPacket),builder_type(name=MultiFragmentPacketBuilder, vis="pub"),mutators(
    pub fn add_fragment(&mut self, fragment_type: u8, data: impl Into<Vec<u8>>) {
        self.fragments.push(BuildFragmentData {
            fragment_type,
            data: data.into()
        });
    }

    /// Add fragments by iterator of `(fragment_type, data)`.
    pub fn add_fragments(&mut self, iter: impl IntoIterator<Item=(u8, impl Into<Vec<u8>>)>) {
        self.fragments.extend(iter.into_iter().map(|(ty, dat)| BuildFragmentData { fragment_type: ty, data: dat.into()}));
    }
    ))]
struct MultiFragmentPacketBuilderInternal {
    #[builder(default = MultiFragmentPacketRef::VALID_MAGIC, setter(prefix="with_"))]
    magic: u16,
    #[builder(setter(prefix = "with_"))]
    event_id: EventId,
    #[builder(setter(prefix = "with_"))]
    source_id: u16,
    #[builder(setter(prefix = "with_"))]
    align: u8,
    #[builder(setter(prefix = "with_"))]
    fragment_version: u8,
    #[builder(default, via_mutators)]
    fragments: Vec<BuildFragmentData>,
}

pub struct BuildFragmentData {
    fragment_type: u8,
    data: Vec<u8>,
}

impl Default for MultiFragmentPacketBuilder {
    fn default() -> Self {
        MultiFragmentPacketBuilderInternal::builder()
    }
}

impl MultiFragmentPacketBuilder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<MultiFragmentPacketBuilderInternal> for crate::MultiFragmentPacket {
    fn from(other: MultiFragmentPacketBuilderInternal) -> Self {
        let header_size = size_of::<MultiFragmentPacketHeader>();
        let fragment_count = other.fragments.len();
        let fragment_count_u16 = u16::try_from(fragment_count).expect("fragment not too large");
        let fragment_types_size =
            alignment_utils::align_up_pow2(fragment_count * size_of::<u8>(), 2);
        let fragment_sizes_size =
            alignment_utils::align_up_pow2(fragment_count * size_of::<u16>(), 2);
        let fragments_size = other.fragments.iter().fold(0, |acc, fragment| {
            acc + alignment_utils::align_up_pow2(fragment.data.len(), other.align)
        });
        let packet_size = header_size + fragment_types_size + fragment_sizes_size + fragments_size;

        // Preallocate the full vector at once with exact size
        let mut data = vec![0u8; packet_size];
        let mut cursor = 0;

        // Write header fields directly into the preallocated buffer
        let write_bytes = |buffer: &mut [u8], offset: &mut usize, bytes: &[u8]| {
            let end = *offset + bytes.len();
            buffer[*offset..end].copy_from_slice(bytes);
            *offset = end;
        };

        write_bytes(&mut data, &mut cursor, &other.magic.to_le_bytes());
        write_bytes(&mut data, &mut cursor, &fragment_count_u16.to_le_bytes());
        write_bytes(
            &mut data,
            &mut cursor,
            &u32::try_from(packet_size)
                .expect("packet size fits u32")
                .to_le_bytes(),
        );
        write_bytes(&mut data, &mut cursor, &other.event_id.to_le_bytes());
        write_bytes(&mut data, &mut cursor, &other.source_id.to_le_bytes());
        write_bytes(&mut data, &mut cursor, &other.align.to_le_bytes());
        write_bytes(
            &mut data,
            &mut cursor,
            &other.fragment_version.to_le_bytes(),
        );

        // Write fragment types
        other.fragments.iter().for_each(|fragment| {
            write_bytes(
                &mut data,
                &mut cursor,
                &fragment.fragment_type.to_le_bytes(),
            );
        });

        // Skip padding for fragment types (already zeroed)
        cursor = header_size + fragment_types_size;

        // Write fragment sizes
        other.fragments.iter().for_each(|fragment| {
            write_bytes(
                &mut data,
                &mut cursor,
                &u16::try_from(fragment.data.len())
                    .expect("fragment size fits u16")
                    .to_le_bytes(),
            );
        });

        // Skip padding for fragment sizes (already zeroed)
        cursor = header_size + fragment_types_size + fragment_sizes_size;

        // Write fragment data
        other.fragments.iter().for_each(|fragment| {
            let fragment_data = &fragment.data;
            write_bytes(&mut data, &mut cursor, fragment_data);

            // Skip padding (already zeroed)
            let aligned_size = alignment_utils::align_up_pow2(fragment.data.len(), other.align);
            cursor = cursor - fragment_data.len() + aligned_size;
        });

        MultiFragmentPacket { data }
    }
}

#[cfg(test)]
mod tests {
    use crate::Fragment;

    use super::*;

    fn demo_multi_fragment_packet_data() -> MultiFragmentPacket {
        MultiFragmentPacketBuilder::new()
            .with_magic(0x40CE)
            .with_event_id(1)
            .with_source_id(1)
            .with_align(3)
            .with_fragment_version(1)
            .add_fragment(0, vec![0, 1, 2, 3])
            .add_fragment(1, vec![0, 1, 2, 3, 4])
            .add_fragment(2, vec![0, 1, 2, 3, 4, 5, 6, 7])
            .add_fragment(3, vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
            .add_fragment(4, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])
            .build()
    }

    #[test]
    fn test_mfp_builder_magic_packet() {
        let mfp = demo_multi_fragment_packet_data();
        assert_eq!(mfp.magic(), 0x40CE);
    }

    #[test]
    fn test_mfp_builder_fragment_count() {
        let mfp = demo_multi_fragment_packet_data();
        assert_eq!(mfp.fragment_count(), 5);
    }

    #[test]
    fn test_mfp_builder_packet_size() {
        let mfp = demo_multi_fragment_packet_data();
        assert_eq!(mfp.raw_packet_data().len(), mfp.packet_size() as usize);
        assert_eq!(mfp.packet_size(), 96);
    }

    #[test]
    fn test_mfp_builder_event_id() {
        let mfp = demo_multi_fragment_packet_data();
        assert_eq!(mfp.event_id(), 1);
    }

    #[test]
    fn test_mfp_builder_source_id() {
        let mfp = demo_multi_fragment_packet_data();
        assert_eq!(mfp.source_id(), 1);
    }

    #[test]
    fn test_mfp_builder_align() {
        let mfp = demo_multi_fragment_packet_data();
        assert_eq!(mfp.align(), 3);
    }

    #[test]
    fn test_mfp_builder_fragments() {
        let mfp = demo_multi_fragment_packet_data();

        dbg!(&mfp.fragment(1));

        let expected_fragments = vec![
            Fragment {
                r#type: 0,
                data: &[0, 1, 2, 3][..],
                version: 1,
                event_id: 1,
                source_id: 1,
            },
            Fragment {
                r#type: 1,
                data: &[0, 1, 2, 3, 4][..],
                version: 1,
                event_id: 2,
                source_id: 1,
            },
            Fragment {
                r#type: 2,
                data: &[0, 1, 2, 3, 4, 5, 6, 7][..],
                version: 1,
                event_id: 3,
                source_id: 1,
            },
            Fragment {
                r#type: 3,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8][..],
                version: 1,
                event_id: 4,
                source_id: 1,
            },
            Fragment {
                r#type: 4,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
                version: 1,
                event_id: 5,
                source_id: 1,
            },
        ];

        let fragments: Vec<Fragment> = mfp.iter().collect();
        assert_eq!(fragments, expected_fragments);
    }
}

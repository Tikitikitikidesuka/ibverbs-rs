use crate::{Fragment, MultiFragmentPacket, MultiFragmentPacketHeader, MultiFragmentPacketRef};
use alignment_utils;
use std::marker::PhantomData;

pub struct MagicDefault;
pub struct MagicSet;
pub struct EventIdNotSet;
pub struct EventIdSet;
pub struct SourceIdNotSet;
pub struct SourceIdSet;
pub struct AlignNotSet;
pub struct AlignSet;
pub struct FragmentVersionNotSet;
pub struct FragmentVersionSet;
pub struct HeaderUnlocked;
pub struct HeaderLocked;

pub struct MultiFragmentPacketBuilder<
    MagicStatus,
    EventIdStatus,
    SourceIdStatus,
    AlignStatus,
    FragmentVersionStatus,
    HeaderLockStatus,
> {
    magic: u16,
    event_id: u64,
    source_id: u16,
    align: u8,
    fragment_version: u8,
    fragments: Vec<Fragment>,
    _typestate_phantom: PhantomData<(
        MagicStatus,
        EventIdStatus,
        SourceIdStatus,
        AlignStatus,
        FragmentVersionStatus,
        HeaderLockStatus,
    )>,
}

impl
    MultiFragmentPacketBuilder<
        MagicDefault,
        EventIdNotSet,
        SourceIdNotSet,
        AlignNotSet,
        FragmentVersionNotSet,
        HeaderUnlocked,
    >
{
    pub fn new() -> Self {
        Self {
            magic: MultiFragmentPacketRef::VALID_MAGIC,
            event_id: 0,
            source_id: 0,
            align: 0,
            fragment_version: 0,
            fragments: Vec::new(),
            _typestate_phantom: PhantomData,
        }
    }
}

impl Default
    for MultiFragmentPacketBuilder<
        MagicDefault,
        EventIdNotSet,
        SourceIdNotSet,
        AlignNotSet,
        FragmentVersionNotSet,
        HeaderUnlocked,
    >
{
    fn default() -> Self {
        Self::new()
    }
}

impl<EventIdStatus, SourceIdStatus, AlignStatus, FragmentVersionStatus>
    MultiFragmentPacketBuilder<
        MagicDefault,
        EventIdStatus,
        SourceIdStatus,
        AlignStatus,
        FragmentVersionStatus,
        HeaderUnlocked,
    >
{
    pub fn with_magic(
        self,
        magic: u16,
    ) -> MultiFragmentPacketBuilder<
        MagicSet,
        EventIdStatus,
        SourceIdStatus,
        AlignStatus,
        FragmentVersionStatus,
        HeaderUnlocked,
    > {
        MultiFragmentPacketBuilder {
            magic,
            event_id: self.event_id,
            source_id: self.source_id,
            align: self.align,
            fragment_version: self.fragment_version,
            fragments: self.fragments,
            _typestate_phantom: PhantomData,
        }
    }
}

impl<MagicStatus, SourceIdStatus, AlignStatus, FragmentVersionStatus>
    MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdNotSet,
        SourceIdStatus,
        AlignStatus,
        FragmentVersionStatus,
        HeaderUnlocked,
    >
{
    pub fn with_event_id(
        self,
        event_id: u64,
    ) -> MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdSet,
        SourceIdStatus,
        AlignStatus,
        FragmentVersionStatus,
        HeaderUnlocked,
    > {
        MultiFragmentPacketBuilder {
            magic: self.magic,
            event_id,
            source_id: self.source_id,
            align: self.align,
            fragment_version: self.fragment_version,
            fragments: self.fragments,
            _typestate_phantom: PhantomData,
        }
    }
}

impl<MagicStatus, EventIdStatus, AlignStatus, FragmentVersionStatus>
    MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdStatus,
        SourceIdNotSet,
        AlignStatus,
        FragmentVersionStatus,
        HeaderUnlocked,
    >
{
    pub fn with_source_id(
        self,
        source_id: u16,
    ) -> MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdStatus,
        SourceIdSet,
        AlignStatus,
        FragmentVersionStatus,
        HeaderUnlocked,
    > {
        MultiFragmentPacketBuilder {
            magic: self.magic,
            event_id: self.event_id,
            source_id,
            align: self.align,
            fragment_version: self.fragment_version,
            fragments: self.fragments,
            _typestate_phantom: PhantomData,
        }
    }
}

impl<MagicStatus, EventIdStatus, SourceIdStatus, FragmentVersionStatus>
    MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdStatus,
        SourceIdStatus,
        AlignNotSet,
        FragmentVersionStatus,
        HeaderUnlocked,
    >
{
    pub fn with_align(
        self,
        align: u8,
    ) -> MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdStatus,
        SourceIdStatus,
        AlignSet,
        FragmentVersionStatus,
        HeaderUnlocked,
    > {
        MultiFragmentPacketBuilder {
            magic: self.magic,
            event_id: self.event_id,
            source_id: self.source_id,
            align,
            fragment_version: self.fragment_version,
            fragments: self.fragments,
            _typestate_phantom: PhantomData,
        }
    }
}

impl<MagicStatus, EventIdStatus, SourceIdStatus, AlignStatus>
    MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdStatus,
        SourceIdStatus,
        AlignStatus,
        FragmentVersionNotSet,
        HeaderUnlocked,
    >
{
    pub fn with_fragment_version(
        self,
        fragment_version: u8,
    ) -> MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdStatus,
        SourceIdStatus,
        AlignStatus,
        FragmentVersionSet,
        HeaderUnlocked,
    > {
        MultiFragmentPacketBuilder {
            magic: self.magic,
            event_id: self.event_id,
            source_id: self.source_id,
            align: self.align,
            fragment_version,
            fragments: self.fragments,
            _typestate_phantom: PhantomData,
        }
    }
}

impl<MagicStatus>
    MultiFragmentPacketBuilder<
        MagicStatus,
        EventIdSet,
        SourceIdSet,
        AlignSet,
        FragmentVersionSet,
        HeaderUnlocked,
    >
{
    pub fn lock_header(
        self,
    ) -> MultiFragmentPacketBuilder<
        MagicSet,
        EventIdSet,
        SourceIdSet,
        AlignSet,
        FragmentVersionSet,
        HeaderLocked,
    > {
        MultiFragmentPacketBuilder {
            magic: self.magic,
            event_id: self.event_id,
            source_id: self.source_id,
            align: self.align,
            fragment_version: self.fragment_version,
            fragments: self.fragments,
            _typestate_phantom: PhantomData,
        }
    }
}

impl
    MultiFragmentPacketBuilder<
        MagicSet,
        EventIdSet,
        SourceIdSet,
        AlignSet,
        FragmentVersionSet,
        HeaderLocked,
    >
{
    pub fn add_fragment(mut self, fragment: Fragment) -> Self {
        self.fragments.push(fragment);
        self
    }

    pub fn add_fragments<I>(mut self, fragments: I) -> Self
    where
        I: IntoIterator<Item = Fragment>,
    {
        self.fragments.extend(fragments);
        self
    }

    // Method can be easily parallelized by calculating indexes instead of extending the vector
    // Probably not worth doing. Overhead will most likely make it slower for average sized MFPs
    pub fn build(self) -> MultiFragmentPacket {
        let header_size = size_of::<MultiFragmentPacketHeader>();
        let fragment_count = self.fragments.len();
        let fragment_types_size =
            alignment_utils::align_up_pow2(fragment_count * size_of::<u8>(), 2);
        let fragment_sizes_size =
            alignment_utils::align_up_pow2(fragment_count * size_of::<u16>(), 2);
        let fragments_size = self.fragments.iter().fold(0, |acc, fragment| {
            acc + alignment_utils::align_up_pow2(fragment.fragment_size() as usize, self.align)
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

        write_bytes(&mut data, &mut cursor, &self.magic.to_le_bytes());
        write_bytes(
            &mut data,
            &mut cursor,
            &(fragment_count as u16).to_le_bytes(),
        );
        write_bytes(&mut data, &mut cursor, &(packet_size as u32).to_le_bytes());
        write_bytes(&mut data, &mut cursor, &self.event_id.to_le_bytes());
        write_bytes(&mut data, &mut cursor, &self.source_id.to_le_bytes());
        write_bytes(&mut data, &mut cursor, &self.align.to_le_bytes());
        write_bytes(&mut data, &mut cursor, &self.fragment_version.to_le_bytes());

        // Write fragment types
        self.fragments.iter().for_each(|fragment| {
            write_bytes(
                &mut data,
                &mut cursor,
                &fragment.fragment_type().to_le_bytes(),
            );
        });

        // Skip padding for fragment types (already zeroed)
        cursor = header_size + fragment_types_size;

        // Write fragment sizes
        self.fragments.iter().for_each(|fragment| {
            write_bytes(
                &mut data,
                &mut cursor,
                &fragment.fragment_size().to_le_bytes(),
            );
        });

        // Skip padding for fragment sizes (already zeroed)
        cursor = header_size + fragment_types_size + fragment_sizes_size;

        // Write fragment data
        self.fragments.iter().for_each(|fragment| {
            let fragment_data = fragment.data();
            write_bytes(&mut data, &mut cursor, fragment_data);

            // Skip padding (already zeroed)
            let aligned_size =
                alignment_utils::align_up_pow2(fragment.fragment_size() as usize, self.align);
            cursor = cursor - fragment_data.len() + aligned_size;
        });

        MultiFragmentPacket { data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FragmentRef;

    fn demo_multi_fragment_packet_data() -> MultiFragmentPacket {
        MultiFragmentPacketBuilder::new()
            .with_magic(0x40CE)
            .with_event_id(1)
            .with_source_id(1)
            .with_align(3)
            .with_fragment_version(1)
            .lock_header()
            .add_fragment(Fragment::new(0, vec![0, 1, 2, 3]).unwrap())
            .add_fragment(Fragment::new(1, vec![0, 1, 2, 3, 4]).unwrap())
            .add_fragment(Fragment::new(2, vec![0, 1, 2, 3, 4, 5, 6, 7]).unwrap())
            .add_fragment(Fragment::new(3, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]).unwrap())
            .add_fragment(Fragment::new(4, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]).unwrap())
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

        let expected_fragments = vec![
            FragmentRef {
                fragment_type: 0,
                fragment_size: 4,
                data: &[0, 1, 2, 3][..],
            },
            FragmentRef {
                fragment_type: 1,
                fragment_size: 5,
                data: &[0, 1, 2, 3, 4][..],
            },
            FragmentRef {
                fragment_type: 2,
                fragment_size: 8,
                data: &[0, 1, 2, 3, 4, 5, 6, 7][..],
            },
            FragmentRef {
                fragment_type: 3,
                fragment_size: 9,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8][..],
            },
            FragmentRef {
                fragment_type: 4,
                fragment_size: 12,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
            },
        ];

        let fragments: Vec<FragmentRef> = mfp.iter().collect();
        assert_eq!(fragments, expected_fragments);
    }
}

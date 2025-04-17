use crate::multi_fragment_packet::mfp::MAGIC_BYTES;
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
            magic: MAGIC_BYTES,
            event_id: 0,
            source_id: 0,
            align: 0,
            fragment_version: 0,
            _typestate_phantom: PhantomData,
        }
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
        mut self,
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
        mut self,
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
        MagicStatus,
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
            _typestate_phantom: PhantomData,
        }
    }
}

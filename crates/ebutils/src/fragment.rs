use std::fmt::{Debug, Display};

use derive_where::derive_where;

use crate::{EventId, fragment_type::FragmentType, source_id::SourceId};

#[derive(PartialEq, Eq)]
#[derive_where(Copy, Clone)]
pub struct Fragment<'a, Data: ?Sized + AsRef<[u8]> = [u8]> {
    r#type: u8,
    version: u8,
    event_id: EventId,
    source_id: SourceId,
    data: &'a Data,
}

impl<'a, T: ?Sized + AsRef<[u8]>> Fragment<'a, T> {
    pub fn new(
        r#type: u8,
        version: u8,
        event_id: EventId,
        source_id: SourceId,
        data: &'a T,
    ) -> Self {
        Fragment {
            r#type,
            version,
            event_id,
            source_id,
            data,
        }
    }

    pub fn fragment_type_raw(&self) -> u8 {
        self.r#type
    }

    pub fn fragment_type_parsed(&self) -> Option<FragmentType> {
        FragmentType::from_repr(self.fragment_type_raw())
    }

    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub fn event_id(&self) -> EventId {
        self.event_id
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn payload(&self) -> &'a T {
        self.data
    }

    pub fn payload_bytes(&self) -> &'a [u8] {
        self.data.as_ref()
    }

    /// in bytes, excluding the header
    #[must_use]
    pub fn fragment_size(&self) -> u16 {
        size_of_val(self.data)
            .try_into()
            .expect("fragment size fits u16")
    }

    pub fn map_payload<U: ?Sized + AsRef<[u8]>>(
        &self,
        f: impl FnOnce(&'a T) -> &'a U,
    ) -> Fragment<'a, U> {
        Fragment {
            r#type: self.r#type,
            version: self.version,
            event_id: self.event_id,
            source_id: self.source_id,
            data: f(self.data),
        }
    }
}

impl Debug for Fragment<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data_preview = if self.data.len() > 16 {
            format!("{:02X?}... ({} bytes)", &self.data[0..16], self.data.len())
        } else {
            format!("{:02X?}", self.data)
        };

        f.debug_struct("Fragment")
            .field("type", &self.r#type)
            .field("size", &self.fragment_size())
            .field("data", &data_preview)
            .field("version", &self.version)
            .field("event_id", &self.event_id)
            .field("source_id", &self.source_id)
            .finish()
    }
}

impl Display for Fragment<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Fragment[type={}, size={}]",
            self.r#type,
            self.fragment_size()
        )
    }
}

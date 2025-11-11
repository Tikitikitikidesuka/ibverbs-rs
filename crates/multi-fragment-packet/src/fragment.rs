use std::fmt::{Debug, Display};

use derive_where::derive_where;

use crate::{EventId, SourceId, fragment_type::FragmentType};

#[derive(PartialEq, Eq)]
#[derive_where(Copy, Clone)]
pub struct Fragment<'a, Data: ?Sized + AsRef<[u8]> = [u8]> {
    pub(crate) r#type: u8,
    pub(crate) version: u8,
    pub(crate) event_id: EventId,
    pub(crate) source_id: SourceId,
    pub(crate) data: &'a Data,
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

    pub fn payload(&self) -> &T {
        self.data
    }

    pub fn payload_bytes(&self) -> &[u8] {
        self.data.as_ref()
    }

    /// in bytes, excluding the header
    #[must_use]
    pub fn fragment_size(&self) -> u16 {
        size_of_val(self.data)
            .try_into()
            .expect("fragment size fits u16")
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

#[cfg(test)]
mod test {
    use crate::{Fragment, MultiFragmentPacket, SourceId};
    fn demo_multi_fragment_packet_data() -> Vec<u8> {
        [
            vec![0xCE, 0x40],                           // Magic (0xCE40)
            vec![5, 0],                                 // Fragment count (5)
            vec![96, 0, 0, 0],                          // Packet size (96)
            vec![1, 0, 0, 0, 0, 0, 0, 0],               //Event id (1)
            vec![1, 0],                                 // Source id (1)
            vec![3],                                    // Align (2^3)
            vec![1],                                    // Fragment version (1)
            vec![0, 1, 2, 3, 4],                        // Fragment types [0, 1, 2, 3, 4]
            vec![0, 0, 0],                              // Padding to 32 bits
            vec![4, 0, 5, 0, 8, 0, 9, 0, 12, 0],        // Fragment sizes [4, 5, 8, 9, 12]
            vec![0, 0],                                 // Padding to 32 bits
            vec![0, 1, 2, 3],                           // Fragment 0
            vec![0, 0, 0, 0],                           // Padding to 2^3
            vec![0, 1, 2, 3, 4],                        // Fragment 1
            vec![0, 0, 0],                              // Padding to 2^3
            vec![0, 1, 2, 3, 4, 5, 6, 7],               // Fragment 2
            vec![],                                     // Padding to 2^3
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8],            // Fragment 3
            vec![0, 0, 0, 0, 0, 0, 0],                  // Padding to 2^3
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], // Fragment 4
            vec![0, 0, 0, 0],                           // Padding to 2^3
        ]
        .concat()
    }

    #[test]
    fn test_mfp_fragment_getter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::ref_from_raw_bytes(&data).unwrap();

        // Check first fragment using direct comparison
        let expected_fragment0 = Fragment {
            r#type: 0,
            version: 1,
            event_id: 1,
            source_id: SourceId(1),
            data: &[0, 1, 2, 3][..],
        };
        assert_eq!(mfp.fragment(0).unwrap(), expected_fragment0);

        // Check last fragment using direct comparison
        let expected_fragment4 = Fragment {
            r#type: 4,
            source_id: SourceId(1),
            event_id: 5,
            version: 1,
            data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
        };
        assert_eq!(mfp.fragment(4).unwrap(), expected_fragment4);

        // Check out of bounds
        assert_eq!(mfp.fragment(5), None);
    }

    #[test]
    fn test_mfp_iter() {
        let data = demo_multi_fragment_packet_data();
        let mfp = MultiFragmentPacket::ref_from_raw_bytes(&data).unwrap();

        let expected_fragments = vec![
            Fragment {
                r#type: 0,
                data: &[0, 1, 2, 3][..],
                version: 1,
                event_id: 1,
                source_id: SourceId(1),
            },
            Fragment {
                r#type: 1,
                data: &[0, 1, 2, 3, 4][..],
                version: 1,
                event_id: 2,
                source_id: SourceId(1),
            },
            Fragment {
                r#type: 2,
                data: &[0, 1, 2, 3, 4, 5, 6, 7][..],
                version: 1,
                event_id: 3,
                source_id: SourceId(1),
            },
            Fragment {
                r#type: 3,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8][..],
                version: 1,
                event_id: 4,
                source_id: SourceId(1),
            },
            Fragment {
                r#type: 4,
                data: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
                version: 1,
                event_id: 5,
                source_id: SourceId(1),
            },
        ];

        let fragments: Vec<Fragment> = mfp.iter().collect();
        assert_eq!(fragments, expected_fragments);
    }
}

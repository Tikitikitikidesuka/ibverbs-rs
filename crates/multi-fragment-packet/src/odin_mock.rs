use ebutils::{FragmentType, OdinPayload, SourceId, odin::UtcDateTime};

use crate::MultiFragmentPacketOwned;

pub struct OdinMock {
    frags_per_mfp: u64,
    source_id: SourceId,
    align_log: u8,
    run_number: u32,
}

impl OdinMock {
    pub fn new(odin_number: u16, run_number: u32, frags_per_mfp: u64, align_log: u8) -> Self {
        Self {
            frags_per_mfp,
            source_id: SourceId::new_odin(odin_number),
            align_log,
            run_number,
        }
    }

    pub fn generate_odin_mfp(&self, mfp_index: u64) -> MultiFragmentPacketOwned {
        let fragments = (0..self.frags_per_mfp).map(|i| {
            let event_id = mfp_index * self.frags_per_mfp + i;
            (
                FragmentType::Odin,
                OdinPayload::builder()
                    .event_id(event_id)
                    .event_type(0)
                    .run_number(self.run_number)
                    .partition_id(0)
                    .gps_time(UtcDateTime::now())
                    .tck(0)
                    .trigger_type(0)
                    .orbit_id(
                        (event_id / OdinPayload::BUNCH_PER_ORBIT as u64)
                            .try_into()
                            .expect("no overflow"),
                    )
                    .bunch_id(
                        (event_id % OdinPayload::BUNCH_PER_ORBIT as u64)
                            .try_into()
                            .expect("no overflow"),
                    )
                    .build()
                    .expect("valid configuration"),
            )
        });

        MultiFragmentPacketOwned::builder()
            .with_align_log(self.align_log)
            .with_source_id(self.source_id)
            .with_event_id(mfp_index * self.frags_per_mfp)
            .with_fragment_version(0)
            .add_fragments(fragments)
            .build()
    }

    pub fn mfp_iterator(&self) -> impl Iterator<Item = MultiFragmentPacketOwned> {
        (0..).map(|i| self.generate_odin_mfp(i))
    }
}

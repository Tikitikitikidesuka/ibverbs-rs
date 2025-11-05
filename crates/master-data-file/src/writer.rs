use std::io::{Result as IoResult, Write};

use multi_event_packet::MultiEventPacketRef;
use multi_fragment_packet::Fragment;

use crate::{MdfHeader, fragment::MdfFragmentHeader};

pub trait WriteMdf {
    fn write_mdf(&self, writer: &mut impl Write) -> IoResult<()>;
}

#[derive(Debug)]
pub struct MdfRecordWriter<'a> {
    fragments: Vec<Fragment<'a>>,
}

impl<'a> MdfRecordWriter<'a> {
    pub fn with_capacity(capacity: usize) -> Self {
        MdfRecordWriter {
            fragments: Vec::with_capacity(capacity),
        }
    }

    pub fn add_fragment(&mut self, frag: Fragment<'a>) {
        self.fragments.push(frag);
    }

    pub fn clear(&mut self) {
        self.fragments.clear();
    }

    pub fn write_and_reset(&mut self, writer: &mut impl Write) -> IoResult<()> {
        let data_size: usize = self
            .fragments
            .iter()
            .map(|f| f.fragment_size() as usize + size_of::<MdfFragmentHeader>())
            .sum();
        let header = MdfHeader::new_simple(data_size);
        writer.write_all(header.as_byets())?;

        for fragment in &self.fragments {
            fragment.write_mdf(writer)?;
        }

        self.clear();
        Ok(())
    }
}

impl WriteMdf for MultiEventPacketRef {
    fn write_mdf(&self, writer: &mut impl Write) -> IoResult<()> {
        let mut record_writer = MdfRecordWriter::with_capacity(self.num_mfps() as _);

        let mut mfp_iterators = self.mfp_iter().map(|mfp| mfp.iter()).collect::<Vec<_>>();

        loop {
            for (idx, iter) in mfp_iterators.iter_mut().enumerate() {
                let Some(frag) = iter.next() else {
                    // some mfp has no more fragments; they should all have the same number; return
                    assert_eq!(
                        idx, 0,
                        "all mfps should have the same number of fragments, in particular the first should not have more"
                    );
                    return Ok(());
                };

                record_writer.add_fragment(frag);
            }
            record_writer.write_and_reset(writer)?;
        }
    }
}

#[cfg(test)]
mod test {
    use multi_event_packet::MultiEventPacket;
    use multi_fragment_packet::MultiFragmentPacket;

    use crate::WriteMdf;

    fn test_writer() {
        let mep = MultiEventPacket::builder()
            .add_mfp(
                MultiFragmentPacket::builder()
                    .with_align(align_of::<u32>() as u8)
                    .with_event_id(0)
                    .with_fragment_version(1)
                    .with_source_id(11)
                    .add_fragments([(1, b"hello".as_ref()), (2, b"how are you?".as_ref())])
                    .build(),
            )
            .unwrap()
            .add_mfp(
                MultiFragmentPacket::builder()
                    .with_align(align_of::<u128>() as u8)
                    .with_event_id(0)
                    .with_fragment_version(22)
                    .with_source_id(2)
                    .build(),
            )
            .unwrap()
            .build();

        let mut mdf = Vec::new();
        mep.write_mdf(&mut mdf).unwrap();
    }
}

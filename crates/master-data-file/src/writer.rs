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

    pub fn reset(&mut self) {
        self.fragments.clear();
    }

    pub fn write_and_reset(&mut self, writer: &mut impl Write) -> IoResult<()> {
        let data_size: usize = self
            .fragments
            .iter()
            .map(|f| {
                size_of::<MdfFragmentHeader>()
                    + (f.fragment_size() as usize).next_multiple_of(align_of::<u32>())
            })
            .sum();

        let header = MdfHeader::new_simple(data_size);
        writer.write_all(header.as_bytes())?;

        for fragment in &self.fragments {
            fragment.write_mdf(writer)?;
        }

        self.reset();
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
    use std::io::Write;

    use multi_event_packet::MultiEventPacket;
    use multi_fragment_packet::MultiFragmentPacket;

    use crate::{
        MdfRecordRef, MdfRecords, WriteMdf,
        header::{SingleEvent, Unknown},
    };

    #[test]
    fn test_writer() {
        struct TraceWriter<W: Write>(W);

        impl<W: Write> Write for TraceWriter<W> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                println!(" - writing {buf:X?}");
                self.0.write(buf)
            }

            fn flush(&mut self) -> std::io::Result<()> {
                self.0.flush()
            }
        }

        let u32_align = align_of::<u32>().ilog2().try_into().unwrap();

        let mep = MultiEventPacket::builder()
            .add_mfp(
                MultiFragmentPacket::builder()
                    .with_align_log(u32_align)
                    .with_event_id(0)
                    .with_fragment_version(1)
                    .with_source_id(11)
                    .add_fragments([(1, b"hello".as_ref()), (2, b"how are you?".as_ref())])
                    .build(),
            )
            .unwrap()
            .add_mfp(
                MultiFragmentPacket::builder()
                    .with_align_log(u32_align)
                    .with_event_id(0)
                    .with_fragment_version(22)
                    .with_source_id(2)
                    .add_fragments([(3, b"bye".as_ref()), (4, b"good, thanks".as_ref())])
                    .build(),
            )
            .unwrap()
            .build();

        let mut mdf = Vec::new();
        mep.write_mdf(&mut TraceWriter(&mut mdf)).unwrap();

        println!("as written {:02X?}", mdf);

        let record = unsafe { &*(mdf.as_ref() as *const [u8] as *const MdfRecordRef<SingleEvent>) };
        println!("{:?}", record.generic_header);

        let records = unsafe { MdfRecords::from_data(&mdf) };
        println!("in record {:08X?}", records.data);
        println!("Records {records:#?}");
        let record = unsafe {
            &*(records
                .data
                .as_ref()
                .as_ptr()
                .cast::<MdfRecordRef<Unknown>>())
        };
        println!("3: {:?}", record.generic_header);
        let records = records
            .mdf_record_iter()
            .map(|r| r.try_into_single_event().unwrap())
            .collect::<Vec<_>>();

        let fragments = records[0].fragments().collect::<Vec<_>>();
        println!("record 0: size {}", records[0].size_u32());
        // sorted by source id...
        assert_eq!(fragments[0].data(), b"bye");
        assert_eq!(fragments[0].fragment_type(), 3);
        assert_eq!(fragments[0].source_id(), 2);

        let fragments = records[1].fragments().collect::<Vec<_>>();
        assert_eq!(fragments[1].data(), b"how are you?");
        assert_eq!(fragments[1].fragment_type(), 2);
        assert_eq!(fragments[1].source_id(), 11);
    }
}

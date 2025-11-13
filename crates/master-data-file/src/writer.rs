use std::io::Write;

use ebutils::{
    fragment::Fragment,
    odin::{FragmentCastError, OdinPayload},
};
use multi_event_packet::MultiEventPacket;
use thiserror::Error;

use crate::{MdfHeader, fragment::MdfFragmentHeader};

/// This is an extension trait to write [`MultiEventPacket`]s as multiple [`MdfRecord`](crate::MdfRecord)s to file.
///
/// # Example
/// ```no_run
/// # use multi_event_packet::MultiEventPacketOwned;
/// use master_data_file::WriteMdf;
///
/// let mdf: MultiEventPacketOwned = todo!();
/// let file = std::fs::File::open("/tmp/test.mdf").unwrap();
/// let buf = std::io::BufWriter::new(file);
/// mdf.write_mdf(&mut buf).unwrap();
/// ```
pub trait WriteMdf {
    /// Writes self in the MDF format using `writer`.
    ///
    /// If using with a [`File`](std::fs::File), consider using a [`std::io::BufWriter`] for better performance.
    fn write_mdf(&self, writer: &mut impl Write) -> Result<(), MdfWriterError>;
}

/// Errors that can occur during writing MDF files.
#[derive(Debug, Error)]
pub enum MdfWriterError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Odin fragment already added for this record")]
    OdinAlreadyAdded,
    #[error("No Odin fragment added for this record")]
    NoOdinFragment,
    #[error("Fragment with type odin could not be cast to Odin payload: {0:?}")]
    BadOdinFragment(FragmentCastError),
}

/// Writes each of this MEP's events to `writer`.
impl WriteMdf for MultiEventPacket {
    fn write_mdf(&self, writer: &mut impl Write) -> Result<(), MdfWriterError> {
        let mut record_writer = MdfRecordWriter::with_capacity(self.num_mfps() as _);

        let mut mfp_iterators = self
            .mfp_iter()
            .map(|mfp| mfp.fragment_iter())
            .collect::<Vec<_>>();

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

                record_writer.add_fragment(frag)?;
            }
            record_writer.write_and_reset(writer)?;
        }
    }
}

#[derive(Debug)]
struct MdfRecordWriter<'a> {
    fragments: Vec<Fragment<'a>>,
    odin: Option<OdinPayload>,
}

impl<'a> MdfRecordWriter<'a> {
    fn with_capacity(capacity: usize) -> Self {
        MdfRecordWriter {
            fragments: Vec::with_capacity(capacity),
            odin: None,
        }
    }

    fn add_fragment(&mut self, frag: Fragment<'a>) -> Result<(), MdfWriterError> {
        match frag.try_into_odin() {
            Ok(odin) => {
                if self.odin.is_some() {
                    return Err(MdfWriterError::OdinAlreadyAdded);
                }
                assert!(frag.fragment_size() as usize == size_of::<OdinPayload>());
                self.odin = Some(*odin.payload());
            }
            Err(e @ FragmentCastError::WrongFragmentSize { .. }) => {
                return Err(MdfWriterError::BadOdinFragment(e));
            }
            Err(_) => {}
        }
        self.fragments.push(frag);

        Ok(())
    }

    fn reset(&mut self) {
        self.fragments.clear();
        self.odin = None;
    }

    fn write_and_reset(&mut self, writer: &mut impl Write) -> Result<(), MdfWriterError> {
        let odin = self.odin.ok_or(MdfWriterError::NoOdinFragment)?;
        let data_size: usize = self
            .fragments
            .iter()
            .map(|f| {
                size_of::<MdfFragmentHeader>()
                    + (f.fragment_size() as usize).next_multiple_of(align_of::<u32>())
            })
            .sum();

        let header = MdfHeader::new_simple(data_size, odin);
        writer.write_all(header.as_bytes())?;

        for fragment in &self.fragments {
            fragment.write_mdf(writer)?;
        }

        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use ebutils::{
        fragment_type::FragmentType,
        odin::OdinPayload,
        source_id::{SourceId, SubDetector},
    };
    use multi_event_packet::MultiEventPacketOwned;
    use multi_fragment_packet::MultiFragmentPacketOwned;

    use crate::{
        MdfRecord, WriteMdf,
        file::MdfFile,
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
        let odin1 = OdinPayload::builder()
            .run_number(42)
            .event_id(0)
            .event_type(7)
            .gps_time(ebutils::odin::UtcDateTime::from_unix_timestamp(1762936178).unwrap())
            .tck(123456)
            .partition_id(0)
            .orbit_id(15)
            .bunch_id(465)
            .trigger_type(5)
            .build()
            .unwrap();

        let mep = MultiEventPacketOwned::builder()
            .add_mfp(
                MultiFragmentPacketOwned::builder()
                    .with_align_log(u32_align)
                    .with_event_id(0)
                    .with_fragment_version(1)
                    .with_source_id(SourceId::new_odin(0))
                    .add_fragment(FragmentType::Odin, odin1)
                    .add_fragment(
                        FragmentType::Odin,
                        OdinPayload::builder()
                            .event_id(1)
                            .run_number(42)
                            .event_type(7)
                            .gps_time(
                                ebutils::odin::UtcDateTime::from_unix_timestamp(1762936178)
                                    .unwrap(),
                            )
                            .tck(123456)
                            .partition_id(0)
                            .orbit_id(15)
                            .bunch_id(455)
                            .trigger_type(5)
                            .build()
                            .unwrap(),
                    )
                    .build(),
            )
            .unwrap()
            .add_mfp(
                MultiFragmentPacketOwned::builder()
                    .with_align_log(u32_align)
                    .with_event_id(0)
                    .with_fragment_version(22)
                    .with_source_id(SourceId::new(SubDetector::VeloC, 55))
                    .add_fragments([
                        (FragmentType::DAQ, b"bye".as_ref()),
                        (FragmentType::Calo, b"good, thanks".as_ref()),
                    ])
                    .build(),
            )
            .unwrap()
            .build()
            .unwrap();

        println!("MEP: {:?}", mep);

        let mut mdf = Vec::new();
        mep.write_mdf(&mut TraceWriter(&mut mdf)).unwrap();

        println!("as written {:02X?}", mdf);

        let record = unsafe { &*(mdf.as_ref() as *const [u8] as *const MdfRecord<SingleEvent>) };
        println!("{:?}", record.generic_header);

        let records = MdfFile::from_data(&mdf);
        println!("in record {:08X?}", records.data());
        println!("Records {records:#?}");
        let record = unsafe {
            &*(records
                .data()
                .as_ref()
                .as_ptr()
                .cast::<MdfRecord<Unknown>>())
        };
        println!("3: {:?}", record.generic_header);
        let records = records
            .mdf_record_iter()
            .map(|r| r.try_into_single_event().unwrap())
            .collect::<Vec<_>>();

        println!("record 0: size {}", records[0].size_u32());
        // sorted by source id...
        let fragments = records[0].fragments().collect::<Vec<_>>();
        assert_eq!(fragments[0].payload(), odin1.as_ref());
        assert_eq!(
            fragments[0].fragment_type_parsed(),
            Some(FragmentType::Odin)
        );
        assert_eq!(fragments[0].source_id().0, 0);

        let fragments = records[1].fragments().collect::<Vec<_>>();
        assert_eq!(fragments[1].payload(), b"good, thanks");
        assert_eq!(
            fragments[1].fragment_type_parsed(),
            Some(FragmentType::Calo)
        );
        assert_eq!(
            fragments[1].source_id(),
            SourceId::new(SubDetector::VeloC, 55)
        );
    }
}

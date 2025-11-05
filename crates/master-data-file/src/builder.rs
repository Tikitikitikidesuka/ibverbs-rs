use std::io::{Result as IoResult, Write};

use multi_fragment_packet::{FragmentRef, SourceId};

use crate::{
    MdfHeader, SingleEvent, SpecificHeaderType,
    fragment::{MdfFragmentHeader, MdfFragmentWriter},
};

// pub trait MdfWriter {
//     fn write_mdf_to(&self, writer: &mut (impl Write + Seek)) -> IoResult<()>;
// }

// impl MdfWriter for MultiEventPacket {
//     fn write_mdf_to(&self, writer: &mut (impl Write + Seek)) -> IoResult<()> {
//         let iter = self.mfp_iter().map(|mfp| mfp.iter()).transpose();
//         while let Some(first) = s.first_mut()
//             && first.peek().is_some()
//         {}

//         let start = writer.stream_position()?;
//         writer.seek_relative(size_of::<MdfGenericHeader>() as i64)?;

//         todo!()
//     }
// }

#[derive(Debug)]
pub struct MdfRecordWriter<'a> {
    event_id: u64,
    source_id: SourceId,
    fragment_version: u8,
    fragments: Vec<&'a FragmentRef<'a>>,
}

impl<'a> MdfRecordWriter<'a> {
    pub fn with_settings_from_mep(capacity: usize) -> Self {
        MdfRecordWriter {
            fragments: Vec::with_capacity(capacity),
            ..todo!()
        }
    }

    pub fn add_fragment(&mut self, frag: &'a FragmentRef<'a>) {
        self.fragments.push(frag);
    }

    pub fn clear(&mut self) {
        self.fragments.clear();
    }

    pub fn write(&mut self, writer: &mut impl Write) -> IoResult<()> {
        let data_size: usize = self
            .fragments
            .iter()
            .map(|f| f.fragment_size() as usize + size_of::<MdfFragmentHeader>())
            .sum();
        let header = MdfHeader::new_simple(data_size);
        writer.write_all(header.as_byets())?;

        for fragment in &self.fragments {
            MdfFragmentWriter::builder()
                .fragment(fragment)
                .version(self.fragment_version)
                .source_id(self.source_id)
                .build()
                .write(writer)?;
        }

        self.clear();
        Ok(())
    }
}

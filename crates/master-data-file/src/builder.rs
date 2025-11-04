use std::io::{Result as IoResult, Write};

use multi_event_packet::MultiEventPacket;
use multi_fragment_packet::FragmentRef;

use crate::{MdfHeader, SingleEvent, SpecificHeaderType, fragment::MdfFragmentHeader};
use std::io::Seek;

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

pub struct MdfBuilder<'a> {
    fragments: Vec<&'a FragmentRef<'a>>,
}

impl<'a> MdfBuilder<'a> {
    pub fn with_capacity(capacity: usize) -> Self {
        MdfBuilder {
            fragments: Vec::with_capacity(capacity),
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
        let header = MdfHeader::new(data_size);
        writer.write_all(header.as_byets())?;

        for fragment in &self.fragments {}

        self.clear();
        Ok(())
    }
}

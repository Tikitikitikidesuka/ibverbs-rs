#![doc = include_str!("../README.md")]

use std::{fmt::Debug, slice};

use bytemuck::cast_ref;
use ebutils::Uninstantiatable;
use ebutils::{EventId, fragment::Fragment, odin::OdinPayload};
use thiserror::Error;

use crate::header::multi_purpose::MultiPurposeType;
use crate::header::{MultiPurpose, SingleEvent};
use crate::{
    fragment::MdfFragment,
    header::{MdfHeader, SpecificHeaderType, Unknown},
};
pub mod file;
mod fragment;
pub mod header;
pub mod rounting_bits;
#[cfg(feature = "mep")]
pub mod writer;

pub use file::MdfFile;
#[cfg(feature = "mep")]
pub use writer::WriteMdf;

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

/// An MDF record is unit of data in an MDF file.
///
/// As MDF files allow for different kinds of data, this struct is generic over a `SpecificHeaderType` which defines the structure of this MDF record.
/// Currently, the following header types exist:
/// - [`Unknown`]: Can represent any header type and is the default when reading from a file.
/// - [`SingleEvent`]: For MDF records containing data for a single event and many sources.
/// - [`MultiPurpose`]: "multi purpose data", currently not well supported.
///
/// The most common use case is to obtain an `MdfRecord<Unknown>`, e.g. form [`MdfFile::mdf_record_iter`],
/// and then try to cast it to an `MdfRecord<SingleEvent>` using [`MdfRecord::try_into_single_event`].
/// On this type, methods like [`MdfRecord::fragments`] are implemented.
///
/// This struct can be thought of similar to [`str`] in a way that it only ever exists behind references `&MdfRecord`, never owned.
///
/// # Example
/// ```rust
/// # use master_data_file::MdfFile;
/// let mdf_file = MdfFile::from_data(include_bytes!("../test.mdf"));
/// for record in mdf_file.mdf_record_iter() {
///     if let Ok(record) = record.try_into_single_event() {
///         for fragment in record.fragments() {
///             // do something with the fragment
///         }
///     }
/// }
/// ```
///
/// The MDF format is defined [here](https://edms.cern.ch/ui/file/784588/2/Online_Raw_Data_Format.pdf#page=5),
/// the single event fragments [here](https://edms.cern.ch/ui/file/565851/5/edms-565851.5.pdf#page=10)
// todo add an external type once they stabilize github.com/rust-lang/rust/issues/43467
#[repr(C, align(4))]
pub struct MdfRecord<H: SpecificHeaderType = Unknown> {
    /// Invariant: sizes are valid (i.e. at least two equal).
    generic_header: MdfHeader<H>,
    _unin: Uninstantiatable,
}

impl<H: SpecificHeaderType> MdfRecord<H> {
    /// Returns the entire record length in units of bytes.
    ///
    /// Note: this is also the way the length is stored in the header in practice, in contrary to what the specification says.
    pub fn size_bytes(&self) -> usize {
        self.generic_header.length_bytes().expect("valid") as _
    }

    /// Returns the entire record length in units of `u32`.
    pub fn size_u32(&self) -> usize {
        self.size_bytes() / size_of::<u32>()
    }

    /// Returns the type of the specific header.
    pub fn specific_header_type(&self) -> u8 {
        self.generic_header.header_type_and_size.header_type()
    }

    fn specific_header_size_bytes(&self) -> usize {
        self.generic_header.header_type_and_size.size_bytes()
    }

    /// Returns the specific header for further inspection.
    pub fn specific_header(&self) -> H {
        self.generic_header.specific_header
    }

    /// Returns the specific header as slice of raw [`u32`].
    ///
    /// Useful for specific header type [`Unknown`].
    pub fn specific_header_raw(&self) -> &[u32] {
        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u32).byte_add(size_of_val(&self.generic_header)),
                self.specific_header_size_bytes() / size_of::<u32>(),
            )
        }
    }

    /// Returns the body of this record, as byte slice.
    pub fn body_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.body_u32())
    }

    /// Returns the body of this record as slice over [`u32`].
    ///
    /// This is possible because an MDF record is always `u32` aligned.
    pub fn body_u32(&self) -> &[u32] {
        // unknown has zero size specific header, account for separately
        let offset = size_of::<MdfHeader<Unknown>>() + self.specific_header_size_bytes();
        assert!(offset.is_multiple_of(size_of::<u32>()));
        let offset32 = offset / size_of::<u32>();

        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u32).add(offset32),
                self.size_u32() - offset32,
            )
        }
    }
}

impl MdfRecord {
    /// This function can create an MDF record from raw data.
    ///
    /// It tries to extract an MDF record from the start of the slice, returning the unused rest.
    /// It fails if the slice is too small or contains invalid MDF length information.
    pub fn from_data(data: &[u32]) -> Result<(&Self, &[u32]), MdfFromDataError> {
        let header_data: &[u32; MdfHeader::<Unknown>::HEADER_SIZE_MIN_U32] = &data
            .split_at_checked(MdfHeader::<Unknown>::HEADER_SIZE_MIN_U32)
            .ok_or(MdfFromDataError::TooSmallForHeader(data.len()))?
            .0
            .try_into()
            .expect("size matches");
        let header: &MdfHeader<Unknown> = cast_ref(header_data);

        let Some(length_32) = header.length_u32() else {
            return Err(MdfFromDataError::HeaderLengthMismatch(header.lengths))?;
        };

        if data.len() < length_32 {
            return Err(MdfFromDataError::TotalLengthMismatch {
                expected: length_32 as _,
                got: data.len(),
            });
        }

        let record = unsafe { &*data.as_ptr().cast() };

        Ok((record, &data[length_32..]))
    }

    /// Tries to convert this record into one of type `SingleEvent`.
    ///
    /// This is necessary to e.g. inspect its fragments.
    pub fn try_into_single_event(&self) -> Result<&MdfRecord<SingleEvent>, HeaderParseError> {
        if self.specific_header_type() != SingleEvent::HEADER_TYPE {
            Err(HeaderParseError::InvalidHeaderType {
                expected: SingleEvent::HEADER_TYPE,
                got: self.specific_header_type(),
            })
        } else if self.specific_header_size_bytes() != size_of::<SingleEvent>() {
            Err(HeaderParseError::InvalidHeaderSize {
                expected: size_of::<SingleEvent>(),
                got: self.specific_header_size_bytes(),
            })
        } else {
            // SAFETY: specific header type and size match that of a single event record.
            Ok(unsafe { &*(self as *const MdfRecord<_> as *const MdfRecord<SingleEvent>) })
        }
    }
}

/// Error indicating why an MDF record could not be read from raw data.
#[derive(Debug, Error)]
pub enum MdfFromDataError {
    #[error("Slice is to small to even read the header: is {0}, but header is at least {hdr} u32 words", hdr = MdfHeader::<Unknown>::HEADER_SIZE_MIN_U32)]
    TooSmallForHeader(usize),
    #[error("Header length do not match: {0:?}")]
    HeaderLengthMismatch([u32; 3]),
    #[error(
        "Header says record has length {expected}, but the slice you provided only has length {got}."
    )]
    TotalLengthMismatch { expected: usize, got: usize },
}

impl Debug for MdfRecord<Unknown> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdfRecordRef")
            .field("generic_header", &self.generic_header)
            .field("body", &truncate_data(self.body_u32()))
            .finish()
    }
}

impl Debug for MdfRecord<SingleEvent> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdfRecordRef")
            .field("generic_header", &self.generic_header)
            .field(
                "fragments",
                &self.fragments().collect::<Vec<_>>().as_slice(),
            )
            .finish()
    }
}

fn truncate_data<'a>(data: &'a [impl Debug]) -> Box<dyn Debug + 'a> {
    if data.len() < 20 {
        Box::new(data)
    } else {
        let mut output = String::new();
        output.push_str("[ ");
        for d in &data[0..10] {
            output.push_str(&format!("{d:?}"));
            output.push_str(", ");
        }
        output.push_str("...");
        output.push_str(" ]");

        Box::new(output)
    }
}

/// Error type indicating why a specific header could not be read from a generic MDF record.
#[derive(Debug, Error)]
pub enum HeaderParseError {
    #[error("Invalid header type: expected {expected} but got {got}")]
    InvalidHeaderType { expected: u8, got: u8 },
    #[error("Invalid header size: expected {expected} but got {got}")]
    InvalidHeaderSize { expected: usize, got: usize },
}

impl<'a> TryFrom<&'a MdfRecord<Unknown>> for &'a MdfRecord<SingleEvent> {
    type Error = HeaderParseError;

    fn try_from(other: &'a MdfRecord<Unknown>) -> Result<&'a MdfRecord<SingleEvent>, Self::Error> {
        other.try_into_single_event()
    }
}

impl MdfRecord<SingleEvent> {
    /// Returns an iterator over the fragments of this single event MDF record.
    pub fn fragments(&self) -> impl Iterator<Item = Fragment<'_>> {
        let event_id = self.odin_fragment().event_id();

        MdfFragmentIterator {
            remaining_data: self.body_u32(),
            event_id,
        }
    }

    /// Returns the odin fragment of this single event MDF record.
    ///
    /// Each single event MDF record must have an ODIN fragment, similar to MEPs.
    /// As fragments are sorted by source id (??) this is just the first fragment.
    pub fn odin_fragment(&self) -> Fragment<'_, OdinPayload> {
        let frag = MdfFragment::from_data(self.body_u32())
            .expect("contains at least one fragment")
            .0;

        let temp_odin = frag
            .as_fragment(EventId::MAX)
            .try_into_odin()
            .expect("First fragment is odin fragment");
        frag.as_fragment(temp_odin.payload().event_id())
            .try_into_odin()
            .expect("First fragment is odin fragment")
    }
}

/// This struct represents an iterator over the fragments of an single event MDF record.
pub struct MdfFragmentIterator<'a> {
    event_id: EventId,
    remaining_data: &'a [u32],
}

impl<'a> Iterator for MdfFragmentIterator<'a> {
    type Item = Fragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_data.is_empty() {
            return None;
        }

        let (frag, rest) = MdfFragment::from_data(self.remaining_data).expect("valid");

        self.remaining_data = rest;

        Some(frag.as_fragment(self.event_id))
    }
}

impl MdfRecord<MultiPurpose> {
    /// Returns the type of this multi purpose mdf record.
    pub fn get_multi_purpose_type(&self) -> Option<MultiPurposeType> {
        MultiPurposeType::from_repr(self.generic_header.data_type)
    }
}

#[cfg(test)]
mod test {

    use include_bytes_aligned::include_bytes_aligned;

    use crate::file::MdfFile;

    #[test]
    #[ignore]
    fn print_data() {
        let file = include_bytes!("../test.mdf");
        // let file = include_bytes!("../../../truc.mdf");
        let records = MdfFile::from_data(file);
        println!("{:#?}", records);
    }

    #[test]
    fn bin_read_test() {
        let file = include_bytes!("../test.mdf");
        let mut cursor = &file[..];

        while !cursor.is_empty() {
            let size = u32::from_le_bytes(cursor[0..4].try_into().unwrap());
            let size2 = u32::from_le_bytes(cursor[4..8].try_into().unwrap());
            let size3 = u32::from_le_bytes(cursor[8..12].try_into().unwrap());

            println!("{size}, {size2}, {size3}");

            assert_eq!(size, size2);
            assert_eq!(size2, size3);

            cursor = &cursor[size as usize..];
        }
    }

    #[test]
    #[ignore]
    fn some_size() {
        let data = include_bytes_aligned!(4, "../test.mdf");
        let mdfs = MdfFile::from_aligned_slice(data).expect("aligned");
        let size: usize = mdfs
            .mdf_record_iter()
            .skip(213)
            .take(2)
            .map(|x| x.size_bytes())
            .sum();
        println!("{size}");
    }

    #[test]
    fn test_file() {
        let records = MdfFile::read_file("test.mdf").unwrap();
        println!("{}", records.mdf_record_iter().count());
    }

    #[test]
    #[cfg(feature = "mmap")]
    fn test_mmap() {
        let records = MdfFile::mmap_file("test.mdf").unwrap();
        println!("{}", records.mdf_record_iter().count());
    }
}

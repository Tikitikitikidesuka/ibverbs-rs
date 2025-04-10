use crate::typed_zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReadable, ZeroCopyRingBufferReadableError, ensure_available_bytes,
};
use crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;
use std::fmt::{Debug, Display};
use std::ops::Index;
use thiserror::Error;

const HEADER_SIZE: usize = size_of::<MultiFragmentPacketRef<'_>>();
const MAGIC_BYTES: u16 = 0x40CE;

#[repr(C, packed)]
pub struct MultiFragmentPacketRef<'a> {
    magic: u16,
    fragment_count: u16,
    packet_size: u16,
    event_id: u32,
    source_id: u16,
    align: u8,
    fragment_version: u8,
    // Array of fragment types is dynamically sized [FragmentType]
    // Array of fragment sizes is dynamically sized [FragmentSize]
    // Array of fragments is dynamically sized [Fragment ([u8])]
    _phantom: std::marker::PhantomData<&'a [u8]>,
}

type FragmentType = u8;
type FragmentSize = u16;

pub struct Fragment<'a> {
    fragment_type: FragmentType,
    fragment_size: FragmentSize,
    data: &'a [u8],
}

#[derive(Debug, Error)]
pub enum MultiFragmentPacketFromRawBytesError {
    #[error(
        "Not enough data available: Required {required_data} bytes. Only {available_data} bytes are available in the buffer"
    )]
    NotEnoughDataAvailable {
        available_data: usize,
        required_data: usize,
    },

    #[error(
        "Magic bytes on the header are corrupted: Expected {expected_magic:x?}, found {read_magic:x?}"
    )]
    CorruptedMagic {
        read_magic: u16,
        expected_magic: u16,
    },
}

impl<'a> MultiFragmentPacketRef<'a> {
    pub fn from_raw_bytes(data: &[u8]) -> Result<&Self, MultiFragmentPacketFromRawBytesError> {
        // Check if there is enough data for the header
        if data.len() < HEADER_SIZE {
            Err(
                MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                    required_data: HEADER_SIZE,
                    available_data: data.len(),
                },
            )?;
        }

        // Cast to MFPRef type to read its attributes
        let mfp = unsafe { &*(data.as_ptr() as *const MultiFragmentPacketRef) };

        // Check the magic bytes are not corrupt
        if mfp.magic() != MAGIC_BYTES {
            Err(MultiFragmentPacketFromRawBytesError::CorruptedMagic {
                read_magic: mfp.magic(),
                expected_magic: MAGIC_BYTES,
            })?
        }

        // Check if there is enough data for the whole packet
        let packet_size = mfp.packet_size() as usize;
        if data.len() < packet_size {
            Err(
                MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                    required_data: HEADER_SIZE,
                    available_data: packet_size,
                },
            )?;
        }

        Ok(mfp)
    }

    pub fn magic(&self) -> u16 {
        self.magic
    }

    pub fn fragment_count(&self) -> u16 {
        self.fragment_count
    }

    pub fn packet_size(&self) -> u16 {
        self.packet_size
    }

    pub fn event_id(&self) -> u32 {
        self.event_id
    }

    pub fn source_id(&self) -> u16 {
        self.source_id
    }

    pub fn align(&self) -> u8 {
        self.align
    }

    pub fn fragment_version(&self) -> u8 {
        self.fragment_version
    }

    pub fn fragment_type(&self, index: usize) -> Option<FragmentType> {
        if index < self.fragment_count() as usize {
            let fragment_type_ptr = unsafe { self.fragment_type_ptr().add(index) };
            let fragment_type = unsafe { *fragment_type_ptr };
            Some(fragment_type)
        } else {
            None
        }
    }

    pub fn fragment_size(&self, index: usize) -> Option<FragmentSize> {
        if index < self.fragment_count() as usize {
            let fragment_size_ptr = unsafe { self.fragment_size_ptr().add(index) };
            let fragment_size = unsafe { *fragment_size_ptr };
            Some(fragment_size)
        } else {
            None
        }
    }

    pub fn fragment_data(&self, index: usize) -> Option<&[u8]> {
        // TODO: FRAGMENTS ARE NOT OF THE SAME SIZE, DUMMY!
        // TODO: YOU GET TO REDO THIS PART TOMORROW >:(
        // TODO: FROM ME TO MYSELF <3
        todo!()
        if index < self.fragment_count() as usize {
            let fragment_data_ptr = unsafe { self.fragment_data_ptr().add(index) };
            let fragment_size = self.fragment_size(index)?;
            Some(unsafe { std::slice::from_raw_parts(fragment_data_ptr, fragment_size as usize) })
        } else {
            None
        }
    }

    pub fn fragment(&self, index: usize) -> Option<Fragment> {
        if index < self.fragment_count() as usize {
            Some(Fragment {
                fragment_type: self.fragment_type(index)?,
                fragment_size: self.fragment_size(index)?,
                data: self.fragment_data(index)?,
            })
        } else {
            None
        }
    }

    pub fn fragment_iter(&self) -> MultiFragmentPacketRefIter {
        MultiFragmentPacketRefIter {
            packet: self,
            index: 0,
        }
    }

    unsafe fn fragment_type_ptr(&self) -> *const FragmentType {
        unsafe { (self as *const Self).add(HEADER_SIZE) as *const FragmentType }
    }

    unsafe fn fragment_size_ptr(&self) -> *const FragmentSize {
        let fragment_types_size = self.fragment_count() as usize * size_of::<FragmentType>();
        unsafe {
            (self as *const Self).add(HEADER_SIZE + fragment_types_size) as *const FragmentSize
        }
    }

    unsafe fn fragment_data_ptr(&self) -> *const u8 {
        let fragment_types_size = self.fragment_count() as usize * size_of::<FragmentType>();
        let fragment_sizes_size = self.fragment_count() as usize * size_of::<FragmentSize>();
        unsafe {
            (self as *const Self).add(HEADER_SIZE + fragment_types_size + fragment_sizes_size)
                as *const u8
        }
    }
}

struct MultiFragmentPacketRefIter<'a> {
    packet: &'a MultiFragmentPacketRef<'a>,
    index: usize,
}

impl<'a> Iterator for MultiFragmentPacketRefIter<'a> {
    type Item = Fragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.packet.fragment(self.index)
    }
}

impl<'buf, R> ZeroCopyRingBufferReadable<'buf, R> for MultiFragmentPacketRef<'buf>
where
    R: ZeroCopyRingBufferReader,
{
    fn load(reader: &mut R, offset: usize) -> Result<usize, ZeroCopyRingBufferReadableError> {
        // Ensure enough data for the header
        ensure_available_bytes(reader, offset + HEADER_SIZE)?;

        // Get temporary access to the data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[offset..(offset + HEADER_SIZE)];
        let mfp = unsafe { &*(header_data.as_ptr() as *const MultiFragmentPacketRef<'_>) };

        // Get the total packet size from the header
        let packet_size = mfp.packet_size() as usize;

        // Ensure enough data for the whole mfp
        ensure_available_bytes(reader, offset + packet_size)?;

        Ok(packet_size)
    }

    fn cast(data: &[u8]) -> Result<&Self, ZeroCopyRingBufferReadableError> {
        MultiFragmentPacketRef::from_raw_bytes(data).map_err(|error| match error {
            MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                required_data: required_bytes,
                available_data: available_bytes,
            } => ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                required_data: required_bytes,
                available_data: available_bytes,
            },
            MultiFragmentPacketFromRawBytesError::CorruptedMagic {
                read_magic,
                expected_magic,
            } => ZeroCopyRingBufferReadableError::ImproperlyFormattedData {
                message: format!(
                    "Expected magic bytes {:x?} but found {:x?}",
                    expected_magic, read_magic
                ),
            },
        })
    }
}

impl<'a> Debug for MultiFragmentPacketRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiFragmentPacketRef")
            .field("magic", &format!("0x{:04X}", self.magic()))
            .field("fragment_count", &self.fragment_count())
            .field("packet_size", &self.packet_size())
            .field("event_id", &self.event_id())
            .field("source_id", &self.source_id())
            .field("align", &self.align())
            .field("fragment_version", &self.fragment_version())
            .finish()
    }
}

impl<'a> Display for MultiFragmentPacketRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MFP[magic=0x{:04X}, fragments={}, packet_size={}, event_id={}, source_id={}, fragment_version={}]",
            self.magic(),
            self.fragment_count(),
            self.packet_size(),
            self.event_id(),
            self.source_id(),
            self.fragment_version()
        )
    }
}

#[derive(Debug)]
struct FragmentDebugView {
    fragment_type: FragmentType,
    fragment_size: FragmentSize,
    data_preview: String,
}

impl<'a> Debug for Fragment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data_preview = if self.data.len() > 16 {
            format!("{:02X?}... ({} bytes)", &self.data[0..16], self.data.len())
        } else {
            format!("{:02X?}", self.data)
        };

        f.debug_struct("Fragment")
            .field("type", &self.fragment_type)
            .field("size", &self.fragment_size)
            .field("data", &data_preview)
            .finish()
    }
}

impl<'a> Display for Fragment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Fragment[type={}, size={}]",
            self.fragment_type, self.fragment_size
        )
    }
}

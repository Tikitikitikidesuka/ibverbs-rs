use std::fmt::{Display, Formatter};
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};

/// Common interface for diary entry types.
///
/// This trait abstracts over different diary entry representations, allowing both buffered
/// (zero-copy from circular buffer) and owned (heap-allocated) variants to be used
/// interchangeably in generic code.
pub trait DiaryEntry {
    /// Returns the day of the month for this entry.
    fn day(&self) -> i32;

    /// Returns the month (1-12) for this entry.
    fn month(&self) -> i32;

    /// Returns the year for this entry.
    fn year(&self) -> i32;

    /// Returns the note content as a string slice.
    fn note(&self) -> &str;
}

/// Wire format representation of a diary entry for circular buffer storage.
///
/// This type uses a packed C representation to match the exact memory layout expected
/// by the buffer protocol. It contains a fixed-size header followed by a variable-length
/// note field that extends beyond the struct bounds.
///
/// # Memory Layout
///
/// ```
/// +------------------+
/// | magic: [u8; 2]   |  2 bytes - corruption detection
/// +------------------+
/// | day: i32         |  4 bytes
/// +------------------+
/// | month: i32       |  4 bytes
/// +------------------+
/// | year: i32        |  4 bytes
/// +------------------+
/// | note_length: u32 |  4 bytes
/// +------------------+
/// | note data...     |  variable length (note_length bytes)
/// +------------------+
/// ```
///
/// The `note` field is not part of the struct itself but follows immediately after
/// the header in memory. Access is provided through the [`note()`](DiaryEntry::note)
/// method which unsafely interprets the bytes following the header.
///
/// # Safety
///
/// This type must only be created by casting from buffer memory that contains a valid
/// entry. Direct construction is unsafe as it would leave the note inaccessible.
#[derive(Debug)]
#[repr(C, packed)]
pub struct BufferedDiaryEntry {
    pub magic: [u8; 2],
    pub day: i32,
    pub month: i32,
    pub year: i32,
    pub note_length: u32,
    // note: rust str as bytes
}

impl BufferedDiaryEntry {
    /// Returns the total size needed to store this entry in a buffer.
    ///
    /// This includes the fixed-size header (`size_of::<Self>()`) plus the variable-length note data.
    pub fn buffered_size(&self) -> usize {
        size_of::<Self>() + self.note_length as usize
    }

    /// Returns the size of the magic bytes field.
    pub const fn magic_bytes_size() -> usize {
        2
    }

    /// Accesses the magic bytes at a given memory address.
    ///
    /// This allows validation of magic numbers before casting the full entry,
    /// enabling early detection of corruption or misalignment.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - `entry_address` points to valid, initialized memory
    /// - At least [`magic_bytes_size()`](Self::magic_bytes_size) bytes are readable at that address
    /// - The memory remains valid for the lifetime `'a`
    pub unsafe fn magic_bytes<'a>(entry_address: *const u8) -> &'a [u8] {
        unsafe { &*slice_from_raw_parts(entry_address, Self::magic_bytes_size()) }
    }

    /// Mutably accesses the magic bytes at a given memory address.
    ///
    /// This is used during write operations to populate magic numbers for newly
    /// created entries.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - `entry_address` points to valid, exclusively owned memory
    /// - At least [`magic_bytes_size()`](Self::magic_bytes_size) bytes are writable at that address
    /// - The memory remains valid and exclusively accessible for the lifetime `'a`
    /// - No other references (mutable or immutable) exist to this memory
    pub unsafe fn magic_bytes_mut<'a>(entry_address: *mut u8) -> &'a mut [u8] {
        unsafe { &mut *slice_from_raw_parts_mut(entry_address, Self::magic_bytes_size()) }
    }
}

impl DiaryEntry for BufferedDiaryEntry {
    fn day(&self) -> i32 {
        self.day
    }

    fn month(&self) -> i32 {
        self.month
    }

    fn year(&self) -> i32 {
        self.year
    }

    fn note(&self) -> &str {
        unsafe {
            let note_ptr = (self as *const Self as *const u8).add(size_of::<Self>());
            let note_slice = std::slice::from_raw_parts(note_ptr, self.note_length as usize);
            std::str::from_utf8_unchecked(note_slice)
        }
    }
}

#[derive(Debug)]
pub struct OwnedDiaryEntry {
    day: i32,
    month: i32,
    year: i32,
    note: String,
}

impl OwnedDiaryEntry {
    pub fn new(day: i32, month: i32, year: i32, note: String) -> Self {
        Self {
            day,
            month,
            year,
            note,
        }
    }
}

impl DiaryEntry for OwnedDiaryEntry {
    fn day(&self) -> i32 {
        self.day
    }

    fn month(&self) -> i32 {
        self.month
    }

    fn year(&self) -> i32 {
        self.year
    }

    fn note(&self) -> &str {
        self.note.as_str()
    }
}

pub trait MockWritable {
    fn buffered_size(&self) -> usize;
}

impl MockWritable for OwnedDiaryEntry {
    fn buffered_size(&self) -> usize {
        size_of::<BufferedDiaryEntry>() + self.note().as_bytes().len()
    }
}

impl MockWritable for BufferedDiaryEntry {
    fn buffered_size(&self) -> usize {
        size_of::<BufferedDiaryEntry>() + self.note().as_bytes().len()
    }
}

pub fn format_diary_entry<T: DiaryEntry>(
    f: &mut Formatter<'_>,
    diary_entry: &T,
) -> std::fmt::Result {
    write!(
        f,
        "[{}/{}/{}] -> ({})",
        diary_entry.day(),
        diary_entry.month(),
        diary_entry.year(),
        diary_entry.note()
    )
}

impl Display for BufferedDiaryEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        format_diary_entry(f, self)
    }
}

impl Display for OwnedDiaryEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        format_diary_entry(f, self)
    }
}

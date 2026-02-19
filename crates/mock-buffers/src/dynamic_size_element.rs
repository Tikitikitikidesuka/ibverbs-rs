use std::fmt::{Display, Formatter};
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};

pub trait DiaryEntry {
    fn day(&self) -> i32;
    fn month(&self) -> i32;
    fn year(&self) -> i32;
    fn note(&self) -> &str;
}

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
    pub fn buffered_size(&self) -> usize {
        size_of::<Self>() + self.note_length as usize
    }

    // Buffer space necessary to contain the magic bytes
    pub const fn magic_bytes_size() -> usize {
        2
    }

    // Access magic bytes of an entry at the specified address
    // This function is unsafe as the memory might not be owned
    pub unsafe fn magic_bytes<'a>(entry_address: *const u8) -> &'a [u8] {
        unsafe { &*slice_from_raw_parts(entry_address, Self::magic_bytes_size()) }
    }

    // Access magic bytes of an entry at the specified address mutably
    // This function is unsafe as the memory might not be owned
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

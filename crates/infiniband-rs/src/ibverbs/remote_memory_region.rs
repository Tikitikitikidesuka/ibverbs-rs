use serde::{Deserialize, Serialize};
use std::ops::{Bound, Range, RangeBounds};

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RemoteMemoryRegion {
    pub addr: u64,
    pub length: usize,
    pub rkey: u32,
}

impl RemoteMemoryRegion {
    pub fn sub_region(&self, range: impl RangeBounds<usize>) -> Option<RemoteMemoryRegion> {
        let range = normalize_range(self.length, range)?;

        Some(RemoteMemoryRegion {
            addr: self.addr.checked_add(range.start.try_into().ok()?)?,
            length: range.end - range.start,
            rkey: self.rkey,
        })
    }
}

/// Returns a sub-`RemoteMemoryRegion` corresponding to a field of a struct stored in the remote region.
///
/// Conceptually, this treats the remote memory region `$mr` as if it were a value of type `$Struct`,
/// and returns the byte range for `$Struct::$field` by computing:
/// - `offset = offset_of!($Struct, $field)`
/// - `length = size_of::<FieldType>()`
/// - `addr = mr.addr + offset`
///
/// # Assumptions / requirements
/// - `$mr` must cover at least the bytes of the field.
/// - `$Struct` must have a stable layout (typically `#[repr(C)]`).
#[macro_export]
macro_rules! remote_field {
    ($mr:expr, $Struct:ident :: $field:ident) => {{
        use crate::ibverbs::remote_memory_region::__private;
        // offset_of! returns usize. We pass it as usize and let the private helper convert it.
        let offset = std::mem::offset_of!($Struct, $field);
        __private::sub_region_of(&$mr, offset, |s: &$Struct| &s.$field)
    }};
}

pub use remote_field;

/// Normalize a range relative to a memory's length.
/// Returns Some(start..end) if valid, None if out of bounds.
fn normalize_range(memory_length: usize, range: impl RangeBounds<usize>) -> Option<Range<usize>> {
    let start = match range.start_bound() {
        Bound::Included(&n) => n,
        Bound::Excluded(&n) => n.checked_add(1)?,
        Bound::Unbounded => 0,
    };

    let end = match range.end_bound() {
        Bound::Included(&n) => n.checked_add(1)?,
        Bound::Excluded(&n) => n,
        Bound::Unbounded => memory_length,
    };

    if start > end || end > memory_length {
        None
    } else {
        Some(start..end)
    }
}

#[doc(hidden)]
pub mod __private {
    use super::RemoteMemoryRegion;

    pub fn sub_region_of<T, U>(
        mr: &RemoteMemoryRegion,
        offset: usize,
        _selector: fn(&T) -> &U,
    ) -> Option<RemoteMemoryRegion> {
        let field_size = std::mem::size_of::<U>();
        let end = offset.checked_add(field_size)?;
        mr.sub_region(offset..end)
    }
}

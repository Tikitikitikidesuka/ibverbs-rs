use serde::{Deserialize, Serialize};

/// A `RemoteMemoryRegion` acts as a handle for One-Sided RDMA (Read/Write) operations.
/// It defines where data should be written to or read from on the remote peer.
///
/// When performing an RDMA Write, the local scatter/gather elements (the sge list in
/// the Work Request) are "stitched" together by the hardware into a single serialized
/// byte-stream. This stream is written contiguously starting at `self.addr`.
///
/// Writing past the bounds of the registered memory region will cause the operation to fail.
///
/// Although the struct contains a `length`, the RDMA hardware only uses the `addr` and `rkey`
/// to execute the transaction. The `length` is stored here strictly for client-side informational
/// purposes.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RemoteMemoryRegion {
    addr: u64,
    length: usize,
    rkey: u32,
}

impl RemoteMemoryRegion {
    /// Creates a new `RemoteMemoryRegion` from its raw components.
    pub fn new(addr: u64, length: usize, rkey: u32) -> Self {
        Self { addr, length, rkey }
    }

    pub fn address(&self) -> u64 {
        self.addr
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn rkey(&self) -> u32 {
        self.rkey
    }

    /// Creates a `RemoteMemoryRegion` derived from `self` that acts as a handle on the remote
    /// memory region, but starting at `offset` bytes from the original address.
    ///
    /// This is useful when you have a large registered buffer and need to target a specific
    /// subsection within it for an RDMA operation.
    ///
    /// The resulting length will be `offset` bytes smaller than the original.
    ///
    /// # Returns
    ///
    /// * `Some(RemoteMemoryRegion)` if the offset is within bounds.
    /// * `None` if the offset exceeds the current length.
    pub fn sub_region(&self, offset: usize) -> Option<RemoteMemoryRegion> {
        if offset > self.length {
            return None;
        }

        Some(RemoteMemoryRegion {
            addr: self.addr.checked_add(offset.try_into().ok()?)?,
            length: self.length - offset,
            rkey: self.rkey,
        })
    }
}

/// Creates a `RemoteMemoryRegion` derived from another one by offsetting it assuming the
/// original one contained an array of type `T` on its first byte, such that the new one
/// contains the N-th element on its first byte.
///
/// It is useful for writing to a specific index in a remote array.
///
/// # Example
/// ```rust
/// # use your_crate::{RemoteMemoryRegion, remote_array_field};
/// # use std::mem::size_of;
/// #
/// # let remote_mr = RemoteMemoryRegion::new(0x1000, 4096, 0xCAFE);
/// #
/// // Assuming remote_mr is a `RemoteMemoryRegion` pointing to a remote memory
/// // region which contains an array of `u64` in its first byte.
/// // Create a `RemoteMemoryRegion` pointing to the sub remote memory region
/// // starting at the first byte of the 5th element.
/// let elem_mr = remote_array_field!(remote_mr, u64, 4).unwrap();
/// ```
#[macro_export]
macro_rules! remote_array_field {
    ($mr:expr, $T:ty, $index:expr) => {{
        let type_size = std::mem::size_of::<$T>();
        let offset = $index * type_size;
        $mr.sub_region(offset)
    }};
}

/// Creates a `RemoteMemoryRegion` derived from another one by offsetting it assuming the
/// original one contained the given struct on its first byte, such that the new one
/// contains the specified field on its first byte.
///
/// It is useful for writing to a concrete struct field remotely.
///
/// # Example
/// ```rust
/// # use your_crate::{RemoteMemoryRegion, remote_struct_field};
/// # use std::mem::offset_of;
/// #
/// #[repr(C)]
/// struct Packet {
///     header: u32,
///     _pad: u32,
///     payload: [u8; 1024],
/// }
///
/// # let remote_mr = RemoteMemoryRegion::new(0x1000, 2048, 0xCAFE);
/// #
/// // Assuming remote_mr is a `RemoteMemoryRegion` pointing to a remote memory
/// // region which contains a `Packet` struct in its first byte.
/// // Create a `RemoteMemoryRegion` pointing to the sub remote memory region
/// // starting at the first byte of the field.
/// let payload_mr = remote_struct_field!(remote_mr, Packet::payload).unwrap();
/// ```
#[macro_export]
macro_rules! remote_struct_field {
    ($mr:expr, $Struct:ident :: $field:ident) => {{
        let offset = std::mem::offset_of!($Struct, $field);
        $mr.sub_region(offset)
    }};
}

/// Creates a `RemoteMemoryRegion` derived from another one by offsetting it assuming the
/// original one contained an array of structs on its first byte, such that the new one
/// contains the specified field of the N-th element on its first byte.
///
/// It is useful for writing to a concrete field of a specific element in a remote array.
///
/// # Example
/// ```rust
/// # use your_crate::{RemoteMemoryRegion, remote_struct_array_field};
/// # use std::mem::{size_of, offset_of};
/// #
/// #[repr(C)]
/// struct Node {
///     id: u32,
///     _pad: u32,
///     data: u64,
/// }
///
/// # let remote_mr = RemoteMemoryRegion::new(0x1000, 4096, 0xCAFE);
/// #
/// // Assuming remote_mr is a `RemoteMemoryRegion` pointing to a remote memory
/// // region which contains an array of `Node` structs in its first byte.
/// // Create a `RemoteMemoryRegion` pointing to the sub remote memory region
/// // starting at the first byte of the field in the 3rd element.
/// let data_mr = remote_struct_array_field!(remote_mr, Node, 2, data).unwrap();
/// ```
#[macro_export]
macro_rules! remote_struct_array_field {
    ($mr:expr, $Struct:ident, $index:expr, $field:ident) => {{
        let struct_size = std::mem::size_of::<$Struct>();
        let field_offset = std::mem::offset_of!($Struct, $field);
        let total_offset = ($index * struct_size) + field_offset;
        $mr.from_offset(total_offset)
    }};
}

pub use remote_array_field;
pub use remote_struct_array_field;
pub use remote_struct_field;

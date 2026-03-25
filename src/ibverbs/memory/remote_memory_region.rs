use serde::{Deserialize, Serialize};

/// A handle to a memory region on a **remote peer**.
///
/// This struct provides the necessary coordinates (Address, Length, RKey) to perform
/// One-Sided RDMA operations (Read/Write) against a remote node.
///
/// # The "Contiguous" Constraint
///
/// Unlike local operations which support Scatter/Gather (stitching fragmented memory together),
/// **remote operations are strictly contiguous**.
///
/// * **Targeting** — You specify a single starting address and a total length.
/// * **Behavior** — The RDMA hardware reads or writes a continuous stream of bytes starting
///   at that virtual address.
///
/// If you need to write to multiple non-contiguous buffers on a remote peer, you must issue
/// multiple distinct RDMA Write operations.
///
/// # Safety and Responsibility
///
/// As discussed in the [memory module](crate::ibverbs::memory), remote memory safety cannot
/// be enforced by the Rust compiler.
///
/// * **Local Safety** — **Safe**. Even if this handle points to invalid memory, issuing an
///   operation using it will only result in an error (or success), but will never corrupt
///   *local* process memory.
/// * **Remote Safety** — **Unsafe**. If you write to a `RemoteMemoryRegion` that has been
///   deallocated on the remote peer, the remote NIC will unknowingly overwrite that memory.
///   **This causes Undefined Behavior on the remote peer.**
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RemoteMemoryRegion {
    addr: u64,
    length: usize,
    rkey: u32,
}

impl RemoteMemoryRegion {
    /// Creates a new `RemoteMemoryRegion` from its raw components.
    ///
    /// This is typically done after receiving these values from a remote peer via an
    /// out-of-band communication channel (like a TCP socket or UD message).
    pub fn new(addr: u64, length: usize, rkey: u32) -> Self {
        Self { addr, length, rkey }
    }

    /// Returns the starting virtual address of the remote memory.
    pub fn address(&self) -> u64 {
        self.addr
    }

    /// Returns the length of the remote memory region.
    ///
    /// **Note**: This value is stored for client-side bounds checking and convenience.
    /// The actual hardware enforcement depends on how the memory was registered on the remote peer.
    pub fn length(&self) -> usize {
        self.length
    }

    /// Returns the Remote Key (rkey) authorizing access to this memory.
    pub fn rkey(&self) -> u32 {
        self.rkey
    }

    /// Creates a generic sub-region derived from this one.
    ///
    /// This acts as a handle to a specific slice of the remote memory, starting at `offset`
    /// bytes from the base address.
    ///
    /// # Returns
    ///
    /// * `Some(RemoteMemoryRegion)` — If `offset <= self.length`. The new length is `self.length - offset`.
    /// * `None` — If `offset > self.length`.
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

    /// Same as [`sub_region`](Self::sub_region) but without client-side bounds checking.
    ///
    /// # Safety
    ///
    /// This is safe from a Rust memory model perspective **locally**. If the calculated address/length
    /// falls outside the actual bounds registered on the remote peer, the RDMA hardware
    /// will reject the operation with a **Remote Access Error**.
    pub fn sub_region_unchecked(&self, offset: usize) -> RemoteMemoryRegion {
        RemoteMemoryRegion {
            addr: self.addr + offset as u64,
            length: self.length - offset,
            rkey: self.rkey,
        }
    }
}

/// Creates a [`RemoteMemoryRegion`] pointing to the N-th element of a remote array.
///
/// Assumes `mr` points to the start of a remote array of type `T`. Returns
/// `Some(RemoteMemoryRegion)` for the element at `index`, or `None` if the
/// byte offset overflows or falls outside the region's bounds.
///
/// # Returns
///
/// * `Some(RemoteMemoryRegion)` — A handle to the `index`-th element.
/// * `None` — If the byte offset overflows, exceeds the region length,
///   or the resulting address overflows.
///
/// # Example
///
/// ```
/// use ibverbs_rs::ibverbs::memory::RemoteMemoryRegion;
/// use ibverbs_rs::remote_array_field;
///
/// // Remote memory contains: [u64; 10]
/// let remote_mr = RemoteMemoryRegion::new(0x1000, 80, 0xABCD);
///
/// // Get a handle to the 5th element (index 4)
/// let elem_mr = remote_array_field!(remote_mr, u64, 4).unwrap();
/// assert_eq!(elem_mr.address(), 0x1020);
/// ```
#[macro_export]
macro_rules! remote_array_field {
    ($mr:expr, $T:ty, $index:expr) => {{
        let type_size = std::mem::size_of::<$T>();
        match ($index).checked_mul(type_size) {
            Some(offset) => $mr.sub_region(offset),
            None => None,
        }
    }};
}

/// Unchecked version of [`remote_array_field!`].
///
/// Skips bounds checking against the region length, but the RDMA hardware will
/// reject out-of-bounds operations with a Remote Access Error.
#[macro_export]
macro_rules! remote_array_field_unchecked {
    ($mr:expr, $T:ty, $index:expr) => {{
        let type_size = std::mem::size_of::<$T>();
        let offset = $index * type_size;
        $mr.sub_region_unchecked(offset)
    }};
}

/// Creates a [`RemoteMemoryRegion`] pointing to a specific field of a remote struct.
///
/// Assumes `mr` points to the start of a remote `Struct`. Uses `offset_of!` to
/// compute the field's byte offset and returns a sub-region handle.
///
/// # Returns
///
/// * `Some(RemoteMemoryRegion)` — A handle to the specified field.
/// * `None` — If the field offset exceeds the region length or the resulting
///   address overflows.
///
/// # Example
///
/// ```
/// use ibverbs_rs::ibverbs::memory::RemoteMemoryRegion;
/// use ibverbs_rs::remote_struct_field;
///
/// #[repr(C)]
/// struct Packet {
///     header: u32,
///     payload: [u8; 1024],
/// }
///
/// // Remote memory contains a `Packet`
/// let remote_mr = RemoteMemoryRegion::new(0x1000, 1028, 0xABCD);
///
/// // Get a handle to the 'payload' field
/// let payload_mr = remote_struct_field!(remote_mr, Packet::payload).unwrap();
/// assert_eq!(payload_mr.address(), 0x1004);
/// ```
#[macro_export]
macro_rules! remote_struct_field {
    ($mr:expr, $Struct:ident :: $field:ident) => {{
        let offset = std::mem::offset_of!($Struct, $field);
        $mr.sub_region(offset)
    }};
}

/// Unchecked version of [`remote_struct_field!`].
///
/// Skips bounds checking against the region length, but the RDMA hardware will
/// reject out-of-bounds operations with a Remote Access Error.
#[macro_export]
macro_rules! remote_struct_field_unchecked {
    ($mr:expr, $Struct:ident :: $field:ident) => {{
        let offset = std::mem::offset_of!($Struct, $field);
        $mr.sub_region_unchecked(offset)
    }};
}

/// Creates a [`RemoteMemoryRegion`] pointing to a specific field within an element of a remote array.
///
/// Assumes `mr` points to a remote array of `Struct`. Combines array indexing with
/// field access: computes `index * size_of::<Struct>() + offset_of!(Struct, field)`.
///
/// # Returns
///
/// * `Some(RemoteMemoryRegion)` — A handle to the specified field within the `index`-th element.
/// * `None` — If the byte offset computation overflows, exceeds the region length,
///   or the resulting address overflows.
///
/// # Example
///
/// ```
/// use ibverbs_rs::ibverbs::memory::RemoteMemoryRegion;
/// use ibverbs_rs::remote_struct_array_field;
///
/// #[repr(C)]
/// struct Node {
///     id: u32,
///     data: u64,
/// }
///
/// // Remote memory contains: [Node; 5]
/// let remote_mr = RemoteMemoryRegion::new(0x1000, 60, 0xABCD);
///
/// // Get a handle to the 'data' field of the 3rd Node (index 2)
/// let data_mr = remote_struct_array_field!(remote_mr, Node, 2, data).unwrap();
/// ```
#[macro_export]
macro_rules! remote_struct_array_field {
    ($mr:expr, $Struct:ident, $index:expr, $field:ident) => {{
        let struct_size = std::mem::size_of::<$Struct>();
        let field_offset = std::mem::offset_of!($Struct, $field);
        match ($index)
            .checked_mul(struct_size)
            .and_then(|o| o.checked_add(field_offset))
        {
            Some(total_offset) => $mr.sub_region(total_offset),
            None => None,
        }
    }};
}

/// Unchecked version of [`remote_struct_array_field!`].
///
/// Skips bounds checking against the region length, but the RDMA hardware will
/// reject out-of-bounds operations with a Remote Access Error.
#[macro_export]
macro_rules! remote_struct_array_field_unchecked {
    ($mr:expr, $Struct:ident, $index:expr, $field:ident) => {{
        let struct_size = std::mem::size_of::<$Struct>();
        let field_offset = std::mem::offset_of!($Struct, $field);
        let total_offset = ($index * struct_size) + field_offset;
        $mr.sub_region_unchecked(total_offset)
    }};
}

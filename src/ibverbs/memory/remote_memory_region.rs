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
/// let elem_mr = remote_array_field!(remote_mr, u64, 4_usize).unwrap();
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
/// let data_mr = remote_struct_array_field!(remote_mr, Node, 2_usize, data).unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_fields() {
        let rmr = RemoteMemoryRegion::new(0x1000, 4096, 0xABCD);
        assert_eq!(rmr.address(), 0x1000);
        assert_eq!(rmr.length(), 4096);
        assert_eq!(rmr.rkey(), 0xABCD);
    }

    #[test]
    fn zero_length_region() {
        let rmr = RemoteMemoryRegion::new(0x2000, 0, 1);
        assert_eq!(rmr.length(), 0);
    }

    #[test]
    fn sub_region_at_zero_offset() {
        let rmr = RemoteMemoryRegion::new(0x1000, 100, 42);
        let sub = rmr.sub_region(0).unwrap();
        assert_eq!(sub.address(), 0x1000);
        assert_eq!(sub.length(), 100);
        assert_eq!(sub.rkey(), 42);
    }

    #[test]
    fn sub_region_at_middle() {
        let rmr = RemoteMemoryRegion::new(0x1000, 100, 42);
        let sub = rmr.sub_region(40).unwrap();
        assert_eq!(sub.address(), 0x1028);
        assert_eq!(sub.length(), 60);
    }

    #[test]
    fn sub_region_at_exact_end() {
        let rmr = RemoteMemoryRegion::new(0x1000, 100, 42);
        let sub = rmr.sub_region(100).unwrap();
        assert_eq!(sub.address(), 0x1064);
        assert_eq!(sub.length(), 0);
    }

    #[test]
    fn sub_region_beyond_end_returns_none() {
        let rmr = RemoteMemoryRegion::new(0x1000, 100, 42);
        assert!(rmr.sub_region(101).is_none());
    }

    #[test]
    fn sub_region_address_overflow_returns_none() {
        let rmr = RemoteMemoryRegion::new(u64::MAX, 100, 42);
        // offset 1 would overflow the u64 address
        assert!(rmr.sub_region(1).is_none());
    }

    #[test]
    fn sub_region_large_offset_returns_none() {
        let rmr = RemoteMemoryRegion::new(0x1000, usize::MAX, 42);
        // address addition will overflow
        assert!(rmr.sub_region(usize::MAX).is_none());
    }

    #[test]
    fn sub_region_unchecked_at_middle() {
        let rmr = RemoteMemoryRegion::new(0x1000, 100, 42);
        let sub = rmr.sub_region_unchecked(40);
        assert_eq!(sub.address(), 0x1028);
        assert_eq!(sub.length(), 60);
        assert_eq!(sub.rkey(), 42);
    }

    #[test]
    fn remote_array_field_index_zero() {
        let rmr = RemoteMemoryRegion::new(0x1000, 80, 0xABCD);
        let elem = remote_array_field!(rmr, u64, 0_usize).unwrap();
        assert_eq!(elem.address(), 0x1000);
        assert_eq!(elem.length(), 80);
    }

    #[test]
    fn remote_array_field_index_mid() {
        let rmr = RemoteMemoryRegion::new(0x1000, 80, 0xABCD);
        let elem = remote_array_field!(rmr, u64, 4_usize).unwrap();
        assert_eq!(elem.address(), 0x1020); // 0x1000 + 4*8
        assert_eq!(elem.length(), 80 - 32);
    }

    #[test]
    fn remote_array_field_out_of_bounds() {
        let rmr = RemoteMemoryRegion::new(0x1000, 16, 0xABCD);
        // 3 * 8 = 24 > 16
        assert!(remote_array_field!(rmr, u64, 3_usize).is_none());
    }

    #[test]
    fn remote_array_field_index_overflow() {
        let rmr = RemoteMemoryRegion::new(0x1000, 1024, 0xABCD);
        // usize::MAX * 8 overflows in checked_mul
        assert!(remote_array_field!(rmr, u64, usize::MAX).is_none());
    }

    #[test]
    fn remote_array_field_unchecked_index_mid() {
        let rmr = RemoteMemoryRegion::new(0x1000, 80, 0xABCD);
        let elem = remote_array_field_unchecked!(rmr, u64, 4_usize);
        assert_eq!(elem.address(), 0x1020);
    }

    #[repr(C)]
    struct TestPacket {
        header: u32,
        payload: [u8; 1024],
    }

    #[test]
    fn remote_struct_field_header() {
        let rmr = RemoteMemoryRegion::new(0x1000, 1028, 0xABCD);
        let field = remote_struct_field!(rmr, TestPacket::header).unwrap();
        assert_eq!(field.address(), 0x1000);
    }

    #[test]
    fn remote_struct_field_payload() {
        let rmr = RemoteMemoryRegion::new(0x1000, 1028, 0xABCD);
        let field = remote_struct_field!(rmr, TestPacket::payload).unwrap();
        assert_eq!(field.address(), 0x1004);
    }

    #[test]
    fn remote_struct_field_out_of_bounds() {
        // Region too small to contain the struct
        let rmr = RemoteMemoryRegion::new(0x1000, 2, 0xABCD);
        assert!(remote_struct_field!(rmr, TestPacket::payload).is_none());
    }

    #[test]
    fn remote_struct_field_unchecked_payload() {
        let rmr = RemoteMemoryRegion::new(0x1000, 1028, 0xABCD);
        let field = remote_struct_field_unchecked!(rmr, TestPacket::payload);
        assert_eq!(field.address(), 0x1004);
    }

    #[repr(C)]
    struct TestNode {
        id: u32,
        data: u64,
    }

    #[test]
    fn remote_struct_array_field_first_element() {
        let rmr = RemoteMemoryRegion::new(0x1000, 240, 0xABCD);
        let field = remote_struct_array_field!(rmr, TestNode, 0_usize, data).unwrap();
        let expected_offset = std::mem::offset_of!(TestNode, data);
        assert_eq!(field.address(), 0x1000 + expected_offset as u64);
    }

    #[test]
    fn remote_struct_array_field_nth_element() {
        let rmr = RemoteMemoryRegion::new(0x1000, 240, 0xABCD);
        let field = remote_struct_array_field!(rmr, TestNode, 2_usize, data).unwrap();
        let node_size = std::mem::size_of::<TestNode>();
        let field_offset = std::mem::offset_of!(TestNode, data);
        let expected = 0x1000u64 + (2 * node_size + field_offset) as u64;
        assert_eq!(field.address(), expected);
    }

    #[test]
    fn remote_struct_array_field_overflow() {
        let rmr = RemoteMemoryRegion::new(0x1000, 240, 0xABCD);
        assert!(remote_struct_array_field!(rmr, TestNode, usize::MAX, data).is_none());
    }

    #[test]
    fn remote_struct_array_field_unchecked_nth_element() {
        let rmr = RemoteMemoryRegion::new(0x1000, 240, 0xABCD);
        let field = remote_struct_array_field_unchecked!(rmr, TestNode, 2_usize, data);
        let node_size = std::mem::size_of::<TestNode>();
        let field_offset = std::mem::offset_of!(TestNode, data);
        let expected = 0x1000u64 + (2 * node_size + field_offset) as u64;
        assert_eq!(field.address(), expected);
    }

    #[test]
    fn serde_round_trip() {
        let rmr = RemoteMemoryRegion::new(0xDEAD_BEEF, 9999, 0x42);
        let json = serde_json::to_string(&rmr).unwrap();
        let restored: RemoteMemoryRegion = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.address(), rmr.address());
        assert_eq!(restored.length(), rmr.length());
        assert_eq!(restored.rkey(), rmr.rkey());
    }
}

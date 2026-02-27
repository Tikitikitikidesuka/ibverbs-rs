use std::io;
use std::ptr::{addr_of, addr_of_mut, read_unaligned, write_unaligned};
use std::slice::from_raw_parts_mut;

/// Shared-memory circular buffer structure.
///
/// This dynamically sized type represents the in-memory layout of the LHCb
/// Event Builder shared memory circular buffer: a packed [`Header`] followed
/// by the raw data region used for producer–consumer communication.
///
/// An instance of this type must be backed by a contiguous byte slice whose:
/// - Base address is aligned to 2^`alignment_pow2` bytes.
/// - Length is exactly [`SharedMemoryBufferStructure::required_mem`]
///   for the chosen `capacity` and `alignment_pow2`.
///
/// All fields are stored in a packed header, so pointer loads/stores use
/// unaligned access helpers.
#[repr(C, packed)]
pub struct SharedMemoryBufferStructure {
    header: Header,
    body: [u8], // header padding + buffer data region
}

/// Header of the shared-memory circular buffer structure.
/// See the shared memory circular buffer specification document for attribute details.
#[derive(Debug)]
#[repr(C, packed)]
struct Header {
    write_ptr: BufferPointer,
    read_ptr: BufferPointer,
    capacity: u64,
    alignment_pow2: u64,
    buf_id: u32,
}

/// A transparent wrapper around the encoded buffer pointer.
///
/// In the shared memory circular buffer, each pointer packs two pieces of
/// information into a single `u64`:
/// - A wrap flag bit, which tracks how many times the pointer has wrapped
///   around the data region modulo 2.
/// - An address field, which stores an aligned offset into the data region,
///   with the least significant bit reserved for the wrap flag.
///
/// This type provides helpers to get and set the wrap flag and address
/// without having to manually manipulate bits.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BufferPointer {
    ptr: u64,
}

impl SharedMemoryBufferStructure {
    /// Returns the number of bytes of memory required to host a buffer
    /// with the given `capacity` and `alignment_pow2`.
    ///
    /// The result accounts for:
    /// - The packed header (`Header`).
    /// - Any padding needed to align the start of the data region up to
    ///   2^`alignment_pow2` bytes.
    /// - The `capacity` bytes of data region.
    pub fn required_mem(capacity: usize, alignment_pow2: u8) -> usize {
        ebutils::align_up_pow2(size_of::<Header>(), alignment_pow2) + capacity
    }

    /// Initialize a raw memory slice as a circular buffer instance.
    ///
    /// This function:
    /// - Verifies that `slice.as_ptr()` is aligned to 2^`alignment_pow2`.
    /// - Verifies that `slice.len()` equals
    ///   [`SharedMemoryCircularBufferStructure::required_mem`](...) for the
    ///   given `capacity` and `alignment_pow2`.
    /// - Writes a fresh [`Header`] with zeroed read/write pointers and the
    ///   configured capacity and alignment.
    /// - Optionally fills the data region with `fill_value`.
    ///
    /// On success it returns a `&mut SharedMemoryCircularBufferStructure`
    /// pointing into the original slice.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that `slice` is the only mutable view of this
    ///   memory while the returned reference exists.
    /// - No other aliasing references (raw or shared) may be used to mutate
    ///   the same memory concurrently.
    pub unsafe fn initialize_mut_slice(
        slice: &mut [u8],
        capacity: usize,
        alignment_pow2: u8,
        fill_value: Option<u8>,
    ) -> io::Result<&mut Self> {
        Self::validate_params(slice.as_ptr(), slice.len(), capacity, alignment_pow2)?;
        Self::initialize_mut_slice_unchecked(slice, capacity, alignment_pow2, fill_value)
    }

    /// Equivalent to [`initialize_mut_slice`] but skips all validation.
    ///
    /// # Safety
    /// - The caller must ensure that `slice` is the only mutable view of this
    ///   memory while the returned reference exists.
    /// - No other aliasing references (raw or shared) may be used to mutate
    ///   the same memory concurrently.
    /// - The caller must guarantee that the address, length, `capacity`, and
    ///   `alignment_pow2` satisfy the same constraints that
    ///   [`initialize_mut_slice`] enforces.
    /// - Violating those invariants is undefined behavior.
    pub unsafe fn initialize_mut_slice_unchecked(
        slice: &mut [u8],
        capacity: usize,
        alignment_pow2: u8,
        fill_value: Option<u8>,
    ) -> io::Result<&mut Self> {
        let shmbuf_ptr = Self::from_mut_slice_unchecked(slice)?;

        unsafe {
            // Initialize the header
            let header_ptr: *mut Header = addr_of_mut!((*shmbuf_ptr).header);
            header_ptr.write(Header::new(capacity, alignment_pow2));
        }

        unsafe {
            // Initialize the padding and the body (optional)
            if let Some(fill_value) = fill_value {
                let body_ptr: *mut u8 = addr_of_mut!((*shmbuf_ptr).body) as *mut u8;
                let body_len = capacity; // you need to track this
                for i in 0..body_len {
                    body_ptr.add(i).write(fill_value);
                }
            }
        }

        Ok(unsafe { &mut *shmbuf_ptr })
    }

    /// Reinterpret an existing raw memory slice as a buffer structure.
    ///
    /// This is intended for attaching to an already-initialized shared buffer
    /// whose header and layout were created by trusted code.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that `slice` is the unique mutable view of
    ///   this memory for the duration of the returned reference. Creating
    ///   other mutable references (directly or indirectly) to the same
    ///   memory while the returned `&mut SharedMemoryCircularBufferStructure`
    ///   exists is undefined behavior.
    /// - The contents of `slice` must represent a valid buffer layout:
    ///   a packed [`Header`] at the start, followed by the data region.
    ///   If the header fields are corrupted, this function may still read them,
    ///   but will reject obvious inconsistencies via `validate_params`.
    ///   It does not attempt to repair invalid data.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - `Err(NotEnoughSpace(..))` if `slice` is too small to hold a header.
    /// - `Err(InvalidAlignment(..))` if the stored `alignment_pow2` does not
    ///   fit into `u8` or if the layout checks in `validate_params` fail.
    /// - `Err(NotEnoughSpace(..))` if `slice.len()` does not match the size
    ///   implied by the header’s `capacity` and `alignment_pow2`.
    pub unsafe fn from_mut_slice(
        slice: &mut [u8],
    ) -> io::Result<&mut SharedMemoryBufferStructure> {
        // Check that the slice is large enough to contain a header
        let available = slice.len();
        let required = size_of::<Header>();
        if available < required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Memory size not enough for header (required: {required}, available: {available})"
                ),
            ));
        }

        // Read the header
        let header = unsafe { &*(slice.as_ptr() as *mut Header) };

        // Get validation attributes
        let capacity = header.capacity;
        let alignment_pow2 = header.alignment_pow2;

        // Check that the alignment power fits u8
        if alignment_pow2 > u8::MAX as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Buffer's alignment is too large (alignment_pow2: {alignment_pow2}, maximum: {})",
                    u8::MAX
                ),
            ));
        }

        // Validate the buffer's parameters
        Self::validate_params(
            slice.as_ptr(),
            available,
            capacity as usize,
            alignment_pow2 as u8,
        )?;

        Self::from_mut_slice_unchecked(slice)
    }

    /// Same as [`from_mut_slice`] but without any validation.
    ///
    /// It is intended for internal use when all invariants have already been
    /// established by earlier checks, or when the caller has out-of-band
    /// guarantees about the layout.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that `slice` was originally allocated and
    ///   initialized according to this type’s layout:
    ///   - The first bytes contain a valid packed [`Header`].
    ///   - The remaining bytes correspond to the data region, with length
    ///     `slice.len() - size_of::<Header>()`.
    ///   - The base address and capacity satisfy the alignment and size
    ///     constraints used when the buffer was created.
    /// - `slice.len()` must be exactly the same total size that was used to
    ///   initialize the buffer. Passing an arbitrary slice, a slice that is
    ///   too small, or one with mismatched capacity/alignment is undefined
    ///   behavior.
    /// - The cast from `*mut [u8]` to this DST currently relies on the
    ///   representation of slice fat pointers in Rust. While this matches
    ///   today’s implementation, it is not formally guaranteed; this should
    ///   be updated to use `ptr::from_raw_parts` once available in stable Rust.
    /// - As with all `&mut` references, the returned reference must be the
    ///   only mutable reference to the underlying memory for its entire lifetime.
    pub unsafe fn from_mut_slice_unchecked(
        slice: &mut [u8],
    ) -> io::Result<&mut SharedMemoryBufferStructure> {
        // DST fat pointers are composed of a pointer to the data start and the length of the trailing slice.
        // Casting from *mut [u8] to a DST with a [u8] tail works because they have the same pointer representation.
        // This is true in practice but not formally guaranteed.
        // TODO: Update the cast to use [`std::ptr::from_raw_parts`](https://doc.rust-lang.org/std/ptr/fn.from_raw_parts.html) once it's stable.
        let shmbuf_ptr = unsafe {
            from_raw_parts_mut(slice.as_mut_ptr(), slice.len() - size_of::<Header>()) as *mut [u8]
                as *mut Self
        };

        Ok(unsafe { &mut *shmbuf_ptr })
    }

    /// Validate that the backing memory satisfies all layout invariants.
    ///
    /// Checks that:
    /// - `address` is aligned to 2^`alignment_pow2`.
    /// - `capacity` is a multiple of 2^`alignment_pow2`.
    /// - `length` equals
    ///   [`SharedMemoryCircularBufferStructure::required_mem`](...) for the
    ///   given `capacity` and `alignment_pow2`.
    ///
    /// Returns `Ok(())` if all checks pass, or a
    /// [`SharedMemoryCircularBufferStructureError`] describing the first violation.
    pub fn validate_params(
        address: *const u8,
        length: usize,
        capacity: usize,
        alignment_pow2: u8,
    ) -> io::Result<()> {
        Self::validate_address_alignment(address, alignment_pow2)?;
        Self::validate_capacity_alignment(capacity, alignment_pow2)?;
        Self::validate_enough_mem_size(length, capacity, alignment_pow2)?;
        Ok(())
    }

    /// Validate that the base address is aligned to 2^`alignment_pow2` bytes.
    ///
    /// Returns:
    /// - `Ok(())` if `address` satisfies the alignment constraint.
    /// - `Err(InvalidAlignment(..))` otherwise.
    pub fn validate_address_alignment(address: *const u8, alignment_pow2: u8) -> io::Result<()> {
        if ebutils::check_alignment_pow2(address as usize, alignment_pow2) == false {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Memory address ({address:p}) not aligned to 2^{alignment_pow2}"),
            ))
        } else {
            Ok(())
        }
    }

    /// Validate that the buffer capacity is a multiple of 2^`alignment_pow2`.
    ///
    /// Returns:
    /// - `Ok(())` if `capacity` is correctly aligned.
    /// - `Err(InvalidAlignment(..))` otherwise.
    pub fn validate_capacity_alignment(capacity: usize, alignment_pow2: u8) -> io::Result<()> {
        if ebutils::check_alignment_pow2(capacity, alignment_pow2) == false {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Capacity ({capacity}) not aligned to 2^{alignment_pow2}"),
            ))
        } else {
            Ok(())
        }
    }

    /// Validate there is enough memory available for the buffer.
    ///
    /// Compares `available` against
    /// [`SharedMemoryCircularBufferStructure::required_mem`](...) computed
    /// from `capacity` and `alignment_pow2`.
    ///
    /// Returns:
    /// - `Ok(())` if `available` equals the required size.
    /// - `Err(NotEnoughSpace(..))` otherwise.
    pub fn validate_enough_mem_size(
        available: usize,
        capacity: usize,
        alignment_pow2: u8,
    ) -> io::Result<()> {
        let size = Self::required_mem(capacity, alignment_pow2);
        if available < size {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Not enough memory for the buffer (required: {size}, available: {available})"
                ),
            ))
        } else {
            Ok(())
        }
    }

    /// Load the raw encoded write pointer using an unaligned read.
    ///
    /// The returned value is the packed representation as stored in memory
    /// (including wrap flag and shifted address bits), not a plain byte
    /// offset.
    pub fn write_ptr(&self) -> BufferPointer {
        unsafe { read_unaligned(addr_of!(self.header.write_ptr)) }
    }

    /// Store the raw encoded write pointer using an unaligned write.
    ///
    /// Only the producer-side code that implements the buffer protocol
    /// should call this.
    pub fn set_write_ptr(&mut self, value: BufferPointer) {
        unsafe { write_unaligned(addr_of_mut!(self.header.write_ptr), value) }
    }

    /// Load the raw encoded read pointer using an unaligned read.
    ///
    /// The returned value is the packed representation as stored in memory
    /// (including wrap flag and shifted address bits), not a plain byte
    /// offset.
    pub fn read_ptr(&self) -> BufferPointer {
        unsafe { read_unaligned(addr_of!(self.header.read_ptr)) }
    }

    /// Store the raw encoded read pointer using an unaligned write.
    ///
    /// Only the consumer-side code that implements the buffer protocol
    /// should call this.
    pub fn set_read_ptr(&mut self, value: BufferPointer) {
        unsafe { write_unaligned(addr_of_mut!(self.header.read_ptr), value) }
    }

    /// Load the buffer capacity (in bytes).
    pub fn capacity(&self) -> u64 {
        unsafe { read_unaligned(addr_of!(self.header.capacity)) }
    }

    /// Load the alignment exponent (ALIGN field).
    ///
    /// The effective alignment is 2^`alignment_pow2` bytes.
    pub fn alignment_pow2(&self) -> u64 {
        unsafe { read_unaligned(addr_of!(self.header.alignment_pow2)) }
    }

    /// Load the deprecated buffer identifier.
    ///
    /// This field is kept for compatibility with the specification but is
    /// not used in the current implementation.
    pub fn buf_id(&self) -> u32 {
        unsafe { read_unaligned(addr_of!(self.header.buf_id)) }
    }
}

impl Header {
    /// Construct a new header with zeroed read/write pointers, the given
    /// `capacity`, and alignment exponent `alignment_pow2`. The `buf_id`
    /// field is initialized to zero.
    fn new(capacity: usize, alignment_pow2: u8) -> Self {
        Self {
            write_ptr: BufferPointer::zero(),
            read_ptr: BufferPointer::zero(),
            capacity: capacity as u64,
            alignment_pow2: alignment_pow2 as u64,
            buf_id: 0,
        }
    }
}

impl BufferPointer {
    /// Bit mask for the wrap flag.
    ///
    /// The address bits occupy all bits except the wrap flag bit.
    /// The stored value is the actual byte address right-shifted by one,
    /// so the most significant bit can be used for the wrap flag.
    const WRAP_MASK: u64 = 1u64 << 63;

    /// Bit mask for the address field.
    ///
    /// The address bits occupy all bits except the wrap flag bit.
    /// The stored value is the actual byte address right-shifted by one.
    const PTR_MASK: u64 = !Self::WRAP_MASK;

    /// Create a new buffer pointer at address zero with wrap flag cleared.
    pub fn zero() -> Self {
        Self { ptr: 0 }
    }

    /// Return the current wrap flag state.
    ///
    /// `true` means the pointer has completed an odd number of full laps
    /// around the data region; `false` means an even number (including zero).
    pub fn wrap_flag(&self) -> bool {
        (self.ptr & Self::WRAP_MASK) != 0
    }

    /// Set the wrap flag state, leaving the address bits unchanged.
    pub fn set_wrap_flag(&mut self, wrap_flag: bool) {
        if wrap_flag {
            self.ptr |= Self::WRAP_MASK;
        } else {
            self.ptr &= !Self::WRAP_MASK;
        }
    }

    /// Return the decoded byte address stored in this pointer.
    ///
    /// The address is recovered by masking out the wrap flag and shifting
    /// left by one bit.
    pub fn address(&self) -> u64 {
        (self.ptr & Self::PTR_MASK) << 1
    }

    /// Set the address field while preserving the wrap flag.
    ///
    /// The `address` is stored right-shifted by one bit, so the caller must
    /// ensure it is aligned to at least 2 bytes (its least significant bit
    /// must be zero), and typically to the buffer’s configured alignment.
    pub fn set_address(&mut self, address: u64) {
        self.ptr = (self.ptr & Self::WRAP_MASK) | ((address >> 1) & Self::PTR_MASK);
    }
}

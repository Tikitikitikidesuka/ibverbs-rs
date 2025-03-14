/// OLD VERSION
use std::ffi::{CString, c_void};
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::slice;
use thiserror::Error;

// Re-export the bindings
use crate::bindings::*;

// Define MFP-related constants
const MFP_MAGIC: u16 = 0x40CE;
const MFP_END_RUN_MAGIC: u32 = 0xFFFFFFFF;

/// Errors that can occur when working with PCIe40 devices
#[derive(Error, Debug)]
pub enum PCIe40Error {
    #[error("Failed to find device: {0}")]
    DeviceNotFound(String),

    #[error("Failed to open device: {0}")]
    DeviceOpenError(String),

    #[error("Failed to open stream")]
    StreamOpenError,

    #[error("Stream not enabled")]
    StreamNotEnabled,

    #[error("Failed to lock stream")]
    StreamLockError,

    #[error("Failed to map buffer")]
    BufferMapError,

    #[error("Failed to get buffer information")]
    BufferInfoError,

    #[error("Corrupted data: {0}")]
    CorruptedData(String),

    #[error("Failed to acknowledge read")]
    AcknowledgeError,

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
}

/// Result type for PCIe40 operations
pub type Result<T> = std::result::Result<T, PCIe40Error>;

/// Header of a Multi-Fragment Packet
#[repr(C, packed)]
pub struct MFPHeader {
    pub magic: u16,          // Magic number (0x40CE)
    pub n_banks: u16,        // Number of fragments/banks in this MFP
    pub packet_size: u32,    // Total packet size in bytes
    pub ev_id: u32,          // Event ID
    pub src_id: u16,         // Source ID
    pub align: u8,           // Alignment (power of 2)
    pub block_version: u8,   // Block version
    // The original structure has bank_types and bank_sizes arrays
    // that follow the header. We'll access those dynamically.
}

impl MFPHeader {
    // Safe accessors for packed fields
    pub fn magic(&self) -> u16 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.magic)) }
    }

    pub fn n_banks(&self) -> u16 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.n_banks)) }
    }

    pub fn packet_size(&self) -> u32 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.packet_size)) }
    }

    pub fn ev_id(&self) -> u32 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.ev_id)) }
    }

    pub fn src_id(&self) -> u16 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.src_id)) }
    }

    pub fn align(&self) -> u8 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.align)) }
    }

    pub fn block_version(&self) -> u8 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.block_version)) }
    }
}

/// Represents a Multi-Fragment Packet
pub struct MFP<'a> {
    data: &'a [u8],
    header: &'a MFPHeader,
    phantom: PhantomData<&'a [u8]>, // Ensures MFP doesn't outlive its buffer
}

impl<'a> MFP<'a> {
    /// Create a new MFP from a byte slice
    ///
    /// # Safety
    /// The caller must ensure that the slice contains a valid MFP structure
    /// with correct alignment and size
    fn new(data: &'a [u8]) -> Result<Self> {
        if data.len() < mem::size_of::<MFPHeader>() {
            return Err(PCIe40Error::CorruptedData("MFP data too short".into()));
        }

        // Get a reference to the header
        let header = unsafe { &*(data.as_ptr() as *const MFPHeader) };

        // Validate magic number
        let magic = header.magic();
        println!("Read magic ({magic}) vs expected magic ({MFP_MAGIC})");
        if magic != MFP_MAGIC {
            return Err(PCIe40Error::CorruptedData(
                format!("Invalid MFP magic: 0x{:04x}", magic)
            ));
        }

        // Validate packet size
        let packet_size = header.packet_size();
        if packet_size as usize > data.len() {
            return Err(PCIe40Error::CorruptedData(
                format!("MFP size mismatch: header says {} but buffer has {}",
                        packet_size, data.len())
            ));
        }

        Ok(MFP {
            data,
            header,
            phantom: PhantomData,
        })
    }

    /// Get the header of this MFP
    pub fn header(&self) -> &MFPHeader {
        self.header
    }

    /// Get the raw data of this MFP
    pub fn data(&self) -> &[u8] {
        let packet_size = self.header.packet_size();
        &self.data[..packet_size as usize]
    }

    /// Check if this is an end-of-run MFP
    pub fn is_end_run(&self) -> bool {
        self.header.ev_id() == MFP_END_RUN_MAGIC
    }

    /// Get bank types array
    pub fn bank_types(&self) -> &[u8] {
        let start = mem::size_of::<MFPHeader>();
        let n_banks = self.header.n_banks();
        let end = start + n_banks as usize;
        &self.data[start..end]
    }

    /// Get bank sizes array
    pub fn bank_sizes(&self) -> &[u32] {
        let n_banks = self.header.n_banks() as usize;
        let types_size = n_banks;
        let start = mem::size_of::<MFPHeader>() + types_size;
        let size_array = unsafe {
            slice::from_raw_parts(
                (self.data.as_ptr().add(start)) as *const u32,
                n_banks
            )
        };
        size_array
    }

    /// Iterator over fragments in this MFP
    pub fn fragments(&self) -> FragmentIterator {
        FragmentIterator::new(self)
    }
}

/// Iterator over fragments in an MFP
pub struct FragmentIterator<'a> {
    mfp: &'a MFP<'a>,
    current_index: usize,
    current_offset: usize,
}

impl<'a> FragmentIterator<'a> {
    fn new(mfp: &'a MFP<'a>) -> Self {
        // Calculate the offset to the first fragment
        let types_size = mfp.header.n_banks() as usize;
        let sizes_size = types_size * mem::size_of::<u32>();
        let first_offset = mem::size_of::<MFPHeader>() + types_size + sizes_size;

        FragmentIterator {
            mfp,
            current_index: 0,
            current_offset: first_offset,
        }
    }
}

impl<'a> Iterator for FragmentIterator<'a> {
    type Item = (&'a [u8], u8); // (fragment data, fragment type)

    fn next(&mut self) -> Option<Self::Item> {
        let n_banks = self.mfp.header.n_banks() as usize;
        if self.current_index >= n_banks {
            return None;
        }

        let fragment_type = self.mfp.bank_types()[self.current_index];
        let fragment_size = self.mfp.bank_sizes()[self.current_index] as usize;

        // Get fragment data
        let fragment_data = &self.mfp.data[self.current_offset..self.current_offset + fragment_size];

        // Calculate next fragment offset with padding
        let alignment = 1 << self.mfp.header.align();
        let padding = (alignment - (fragment_size % alignment)) % alignment;
        self.current_offset += fragment_size + padding;
        self.current_index += 1;

        Some((fragment_data, fragment_type))
    }
}

/// Safe Rust wrapper for PCIe40 device
pub struct PCIe40Reader {
    id: i32,
    id_fd: i32,
    stream_fd: i32,
    buffer: *mut c_void,
    buffer_size: usize,
    device_read_offset: usize,
    internal_read_offset: usize,
    requested_size: usize,
    next_ev_id: u32,
    name: String,
}

impl PCIe40Reader {
    /// Open a PCIe40 device by name with MAIN stream
    pub fn open(name: &str) -> Result<Self> {
        Self::open_with_stream(name, P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN as i32)
    }

    /// Open a PCIe40 device by name with specific stream
    pub fn open_with_stream(name: &str, stream: i32) -> Result<Self> {
        let c_name = CString::new(name).map_err(|_| {
            PCIe40Error::DeviceNotFound("Invalid device name".into())
        })?;

        // Find device by name
        let device_id = unsafe { p40_id_find(c_name.as_ptr()) };
        if device_id < 0 {
            return Err(PCIe40Error::DeviceNotFound(name.to_string()));
        }

        Self::open_by_id(device_id, stream)
    }

    /// Open a PCIe40 device by ID
    pub fn open_by_id(id: i32, stream: i32) -> Result<Self> {
        // Open device ID
        let id_fd = unsafe { p40_id_open(id) };
        if id_fd < 0 {
            return Err(PCIe40Error::DeviceOpenError(format!("ID: {}", id)));
        }

        // Open stream
        let stream_fd = unsafe { p40_stream_open(id, stream) };
        if stream_fd < 0 {
            unsafe { p40_id_close(id_fd) };
            return Err(PCIe40Error::StreamOpenError);
        }

        // Check if stream is enabled
        let enabled = unsafe { p40_stream_enabled(stream_fd) };
        if enabled <= 0 {
            unsafe {
                p40_stream_close(stream_fd, ptr::null_mut());
                p40_id_close(id_fd);
            }
            return Err(PCIe40Error::StreamNotEnabled);
        }

        // Lock stream
        if unsafe { p40_stream_lock(stream_fd) } < 0 {
            unsafe {
                p40_stream_close(stream_fd, ptr::null_mut());
                p40_id_close(id_fd);
            }
            return Err(PCIe40Error::StreamLockError);
        }

        // Get buffer size
        let buffer_size = unsafe { p40_stream_get_host_buf_bytes(stream_fd) };
        if buffer_size < 0 {
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, ptr::null_mut());
                p40_id_close(id_fd);
            }
            return Err(PCIe40Error::BufferInfoError);
        }

        // Map buffer
        let buffer = unsafe { p40_stream_map(stream_fd) };
        if buffer.is_null() {
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, ptr::null_mut());
                p40_id_close(id_fd);
            }
            return Err(PCIe40Error::BufferMapError);
        }

        // Get read offset
        let device_read_offset = unsafe { p40_stream_get_host_buf_read_off(stream_fd) };
        if device_read_offset < 0 {
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, buffer);
                p40_id_close(id_fd);
            }
            return Err(PCIe40Error::BufferInfoError);
        }

        // Get device name
        let mut name_buf = [0u8; 256];
        if unsafe { p40_id_get_name_unique(id_fd, name_buf.as_mut_ptr() as *mut i8, name_buf.len()) } != 0 {
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, buffer);
                p40_id_close(id_fd);
            }
            return Err(PCIe40Error::DeviceOpenError("Failed to get device name".into()));
        }

        // Convert name to string
        let name_end = name_buf.iter().position(|&c| c == 0).unwrap_or(name_buf.len());
        let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();

        Ok(PCIe40Reader {
            id,
            id_fd,
            stream_fd,
            buffer,
            buffer_size: buffer_size as usize,
            device_read_offset: device_read_offset as usize,
            internal_read_offset: device_read_offset as usize,
            requested_size: 0,
            next_ev_id: 0,
            name,
        })
    }

    /// Configure MFP mode with given packing factor
    pub fn configure_mfp(&mut self, packing_factor: i32) -> Result<()> {
        // Enable meta stream
        let meta_fd = unsafe { p40_stream_open(self.id, P40_DAQ_STREAM_P40_DAQ_STREAM_META as i32) };
        if meta_fd < 0 {
            return Err(PCIe40Error::StreamOpenError);
        }

        // Get meta mask
        let meta_mask = unsafe { p40_stream_id_to_meta_mask(self.id, self.stream_fd) };

        // Enable meta mask
        if unsafe { p40_stream_enable_mask(meta_fd, meta_mask) } != 0 {
            unsafe { p40_stream_close(meta_fd, ptr::null_mut()) };
            return Err(PCIe40Error::StreamOpenError);
        }

        // Set packing factor
        if unsafe { p40_stream_set_meta_packing(self.stream_fd, packing_factor) } != 0 {
            unsafe { p40_stream_close(meta_fd, ptr::null_mut()) };
            return Err(PCIe40Error::StreamOpenError);
        }

        // Open control stream
        let ctrl_fd = unsafe { p40_ctrl_open(self.id) };
        if ctrl_fd < 0 {
            unsafe { p40_stream_close(meta_fd, ptr::null_mut()) };
            return Err(PCIe40Error::StreamOpenError);
        }

        // Get spill buffer size as truncation threshold
        let trunc_thr = unsafe { p40_ctrl_get_spill_buf_size(ctrl_fd, self.stream_fd) };

        // Set truncation threshold
        if unsafe { p40_ctrl_set_trunc_thres(ctrl_fd, self.stream_fd, trunc_thr) } != 0 {
            unsafe {
                p40_ctrl_close(ctrl_fd);
                p40_stream_close(meta_fd, ptr::null_mut());
            };
            return Err(PCIe40Error::StreamOpenError);
        }

        // Close the streams we don't need to keep open
        unsafe {
            p40_ctrl_close(ctrl_fd);
            p40_stream_close(meta_fd, ptr::null_mut());
        }

        Ok(())
    }

    /// Update available data count
    fn update_usage(&mut self) -> Result<()> {
        let available = unsafe { p40_stream_get_host_buf_bytes_used(self.stream_fd) };
        if available < 0 {
            return Err(PCIe40Error::BufferInfoError);
        }

        Ok(())
    }

    /// Try to read an MFP from the device
    pub fn try_read_mfp(&mut self) -> Result<Option<MFP>> {
        self.update_usage()?;

        // Get available data
        let available = unsafe { p40_stream_get_host_buf_bytes_used(self.stream_fd) } as usize;
        if available <= self.requested_size {
            return Ok(None); // No new data available
        }

        let available_new = available - self.requested_size;

        // Check if we have enough data for at least the MFP header
        if available_new < mem::size_of::<MFPHeader>() {
            return Ok(None);
        }

        // Print buffer status for debugging
        println!("DEBUG: Buffer stats - total: {} bytes, available: {} bytes, requested: {} bytes",
                 self.buffer_size, available, self.requested_size);
        println!("DEBUG: Read position - device_offset: {}, internal_offset: {}",
                 self.device_read_offset, self.internal_read_offset);

        // FIXED: Get a pointer to the current read position
        let buffer_ptr = self.buffer as *const u8;
        let data_ptr = unsafe { buffer_ptr.add(self.internal_read_offset) };

        // Print the first 64 bytes at the current read position for debugging
        let debug_size = std::cmp::min(64, available_new);
        let debug_data = unsafe { slice::from_raw_parts(data_ptr, debug_size) };
        println!("DEBUG: First {} bytes at offset {}:", debug_size, self.internal_read_offset);

        // Print as hex dump with both hex and ASCII representation
        for (i, chunk) in debug_data.chunks(16).enumerate() {
            let hex_str: Vec<String> = chunk.iter()
                .map(|b| format!("{:02x}", b))
                .collect();

            let ascii_str: String = chunk.iter()
                .map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' })
                .collect();

            println!("DEBUG: {:04x}: {:48} | {}",
                     i * 16,
                     hex_str.join(" "),
                     ascii_str);
        }

        // Access header without creating a reference to packed fields
        let header = unsafe { &*(data_ptr as *const MFPHeader) };

        // Validate magic number using safe accessor
        let magic = header.magic();
        if magic != MFP_MAGIC {
            println!("DEBUG: Header fields - magic: 0x{:04x}, n_banks: {}, packet_size: {}, ev_id: 0x{:08x}",
                     magic, header.n_banks(), header.packet_size(), header.ev_id());

            // Look for MFP magic in the next several bytes to check for alignment issues
            println!("DEBUG: Searching for MFP magic in the next 64 bytes...");
            for offset in 0..std::cmp::min(64, available_new-2) {
                let potential_magic = unsafe {
                    let magic_ptr = data_ptr.add(offset) as *const u16;
                    std::ptr::read_unaligned(magic_ptr)
                };
                if potential_magic == MFP_MAGIC {
                    println!("DEBUG: Found MFP magic at offset +{} bytes", offset);
                }
            }

            return Err(PCIe40Error::CorruptedData(
                format!("Invalid MFP magic: 0x{:04x} at buffer offset {}", magic, self.internal_read_offset)
            ));
        }

        // Check if the whole MFP is available
        let packet_size = header.packet_size() as usize;
        println!("DEBUG: Found valid MFP header - magic: 0x{:04x}, size: {} bytes, fragments: {}",
                 magic, packet_size, header.n_banks());

        if available_new < packet_size {
            println!("DEBUG: Not enough data for complete MFP (need {} bytes, have {} bytes)",
                     packet_size, available_new);
            return Ok(None); // Not enough data for the complete MFP
        }

        // Create a view of the entire MFP
        let mfp_data = unsafe { slice::from_raw_parts(data_ptr, packet_size) };

        // Create MFP object
        let mfp = MFP::new(mfp_data)?;

        // Update read position
        let align = header.align();
        let alignment = 1 << align;
        let padding = (alignment - (packet_size % alignment)) % alignment;
        let total_size = packet_size + padding;

        println!("DEBUG: Advancing read position by {} bytes (packet: {} + padding: {})",
                 total_size, packet_size, padding);

        self.internal_read_offset += total_size;
        if self.internal_read_offset >= self.buffer_size {
            println!("DEBUG: Buffer wrap-around - old offset: {}, new offset: {}",
                     self.internal_read_offset, self.internal_read_offset - self.buffer_size);
            self.internal_read_offset -= self.buffer_size; // Wrap around
        }
        self.requested_size += total_size;

        // Verify event ID sequence
        if mfp.is_end_run() {
            println!("DEBUG: End-of-run MFP detected, resetting event ID counter");
            self.next_ev_id = 0; // Reset event ID counter on end of run
        } else {
            let ev_id = header.ev_id();
            if ev_id != self.next_ev_id {
                return Err(PCIe40Error::CorruptedData(
                    format!("Event ID mismatch: expected {}, got {}",
                            self.next_ev_id, ev_id)
                ));
            } else {
                self.next_ev_id += header.n_banks() as u32;
                println!("DEBUG: Updated next expected event ID to {}", self.next_ev_id);
            }
        }

        Ok(Some(mfp))
    }

    /// Acknowledge processed data
    pub fn acknowledge_read(&mut self) -> Result<()> {
        if self.requested_size == 0 {
            return Ok(());
        }

        // Acknowledge data
        if unsafe { p40_stream_free_host_buf_bytes(self.stream_fd, self.requested_size) } < 0 {
            return Err(PCIe40Error::AcknowledgeError);
        }

        // Update device read offset
        let new_offset = unsafe { p40_stream_get_host_buf_read_off(self.stream_fd) };
        if new_offset < 0 {
            return Err(PCIe40Error::BufferInfoError);
        }

        // Reset internal counters
        self.device_read_offset = new_offset as usize;
        self.internal_read_offset = self.device_read_offset;
        self.requested_size = 0;

        Ok(())
    }

    /// Reset the device and reader state
    pub fn reset(&mut self) -> Result<()> {
        // Reset stream logic
        if unsafe { p40_stream_reset_logic(self.stream_fd) } != 0 {
            return Err(PCIe40Error::StreamOpenError);
        }

        // Update device read offset
        let new_offset = unsafe { p40_stream_get_host_buf_read_off(self.stream_fd) };
        if new_offset < 0 {
            return Err(PCIe40Error::BufferInfoError);
        }

        // Reset internal counters
        self.device_read_offset = new_offset as usize;
        self.internal_read_offset = self.device_read_offset;
        self.requested_size = 0;
        self.next_ev_id = 0;

        Ok(())
    }

    /// Get buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Get buffer occupancy
    pub fn buffer_occupancy(&self) -> Result<usize> {
        let available = unsafe { p40_stream_get_host_buf_bytes_used(self.stream_fd) };
        if available < 0 {
            return Err(PCIe40Error::BufferInfoError);
        }

        Ok(available as usize)
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }
}

// Automatic resource cleanup
impl Drop for PCIe40Reader {
    fn drop(&mut self) {
        unsafe {
            p40_stream_unlock(self.stream_fd);
            p40_stream_close(self.stream_fd, self.buffer);
            p40_id_close(self.id_fd);
        }
    }
}

// Example of using the safe wrapper
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_and_read() {
        // This is just an example, not a real test
        let mut reader = PCIe40Reader::open("tdtel203_0")
            .expect("Failed to open PCIe40 device");

        // Configure MFP mode with packing factor 1
        reader.configure_mfp(1).expect("Failed to configure MFP mode");

        // Read and process MFPs in a loop
        let mut mfp_count = 0;

        while mfp_count < 10 {
            match reader.try_read_mfp() {
                Ok(Some(mfp)) => {
                    println!("Read MFP with event ID: {}", mfp.header().ev_id());

                    // Process MFP fragments
                    for (idx, (fragment, frag_type)) in mfp.fragments().enumerate() {
                        println!("  Fragment {}: type {}, size {} bytes",
                                 idx, frag_type, fragment.len());
                    }

                    // Check for end-of-run
                    if mfp.is_end_run() {
                        println!("End of run detected");
                        break;
                    }

                    mfp_count += 1;
                },
                Ok(None) => {
                    // No data available, wait a bit
                    std::thread::sleep(std::time::Duration::from_millis(10));
                },
                Err(e) => {
                    eprintln!("Error reading MFP: {}", e);
                    break;
                }
            }

            // Acknowledge processed data periodically
            if mfp_count % 5 == 0 {
                reader.acknowledge_read().expect("Failed to acknowledge read");
            }
        }

        // Final acknowledge
        reader.acknowledge_read().expect("Failed to acknowledge read");
    }
}
// lib.rs
use std::error::Error;
use std::ffi::CString;
use std::fmt;

// Suppress warnings about non-standard naming in imported C bindings
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
// Make bindings public so main.rs can use them
pub mod bindings;
//pub mod mfp_reader;
//pub mod mfp_ref;
//pub mod pcie40_mfp_reader;
pub mod pcie40_reader;
pub mod zero_copy_reader_old;
pub mod pcie40_id;
pub mod pcie40_ctrl;
pub mod pcie40_stream;
pub mod zero_copy_reader;
/*
// Error handling for PCIe40 operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PCIe40Error {
    IdFind,
    IdOpen,
    MetaStreamOpen,
    StreamOpen,
    StreamGetEnabled,
    StreamEnabled,
    StreamLock,
    StreamSize,
    StreamMap,
    CtrlStreamOpen,
    DmabufOpen,
    DeviceNotOpen,
    HostReadOff,
    GetName,
    SetMetaPacking,
    MetaStreamEnable,
    MetaStreamDisable,
    InvalidTruncationThreshold,
    LogicReset,
    CorruptedData,
    CorruptedEvId,
    HostBytesUsed,
    HostFreeBuff,
    InvalidPackingFactor,
    InternalBufferFull,
    InternalBufferFullFlush,
}

impl fmt::Display for PCIe40Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PCIe40Error::IdFind => write!(f, "Failed to find PCIe ID"),
            PCIe40Error::IdOpen => write!(f, "Failed to open PCIe ID"),
            PCIe40Error::MetaStreamOpen => write!(f, "Failed to open META stream"),
            PCIe40Error::StreamOpen => write!(f, "Failed to open stream"),
            PCIe40Error::StreamGetEnabled => write!(f, "Failed to get stream enabled status"),
            PCIe40Error::StreamEnabled => write!(f, "Stream not enabled"),
            PCIe40Error::StreamLock => write!(f, "Failed to lock stream"),
            PCIe40Error::StreamSize => write!(f, "Failed to get stream size"),
            PCIe40Error::StreamMap => write!(f, "Failed to map stream"),
            PCIe40Error::CtrlStreamOpen => write!(f, "Failed to open control stream"),
            PCIe40Error::DmabufOpen => write!(f, "Failed to open DMABUF"),
            PCIe40Error::DeviceNotOpen => write!(f, "Device not open"),
            PCIe40Error::HostReadOff => write!(f, "Failed to get host read offset"),
            PCIe40Error::GetName => write!(f, "Failed to get device name"),
            PCIe40Error::SetMetaPacking => write!(f, "Failed to set META packing"),
            PCIe40Error::MetaStreamEnable => write!(f, "Failed to enable META stream"),
            PCIe40Error::MetaStreamDisable => write!(f, "Failed to disable META stream"),
            PCIe40Error::InvalidTruncationThreshold => write!(f, "Invalid truncation threshold"),
            PCIe40Error::LogicReset => write!(f, "Failed to reset logic"),
            PCIe40Error::CorruptedData => write!(f, "Corrupted data"),
            PCIe40Error::CorruptedEvId => write!(f, "Corrupted event ID"),
            PCIe40Error::HostBytesUsed => write!(f, "Failed to get host bytes used"),
            PCIe40Error::HostFreeBuff => write!(f, "Failed to free host buffer"),
            PCIe40Error::InvalidPackingFactor => write!(f, "Invalid packing factor"),
            PCIe40Error::InternalBufferFull => write!(f, "Internal buffer full"),
            PCIe40Error::InternalBufferFullFlush => write!(f, "Internal buffer full during flush"),
        }
    }
}

impl Error for PCIe40Error {}

// Basic implementation of PCIe40 reader
pub struct PCIe40Reader {
    id: i32,
    stream: i32,  // Using i32 instead of enum
    id_fd: i32,
    stream_fd: i32,
    ctrl_fd: i32,
    meta_fd: i32,
    dmabuf_fd: i32,
    buffer: *mut std::ffi::c_void,
    buffer_size: usize,
    internal_read_off: usize,
    device_read_off: usize,
    available_data: usize,
    requested_size: usize,
    next_ev_id: u32,
    name: String,
    pub src_id: i32,
    block_version: i32,
}

// Constants similar to C++ code
const DEFAULT_NEXT_EV_ID: u32 = 0;
const PCIE40_UNIQUE_NAME_STR_LENGTH: usize = 128;
const SRC_NUM_MASK: i32 = 0x00FFFF00;
const SRC_SUBSYSTEM_MASK: i32 = 0x00FFFF00;
const SRC_VERSION_MASK: i32 = 0x000000FF;
const SRC_NUM_SHIFT: i32 = 8;
const SRC_VERSION_SHIFT: i32 = 0;

impl PCIe40Reader {
    pub fn new() -> Self {
        Self {
            id: -1,
            stream: bindings::P40_DAQ_STREAM_P40_DAQ_STREAM_NULL,
            id_fd: -1,
            stream_fd: -1,
            ctrl_fd: -1,
            meta_fd: -1,
            dmabuf_fd: -1,
            buffer: std::ptr::null_mut(),
            buffer_size: 0,
            internal_read_off: 0,
            device_read_off: 0,
            available_data: 0,
            requested_size: 0,
            next_ev_id: DEFAULT_NEXT_EV_ID,
            name: String::from("undefined_device"),
            src_id: 0,
            block_version: 0,
        }
    }

    pub fn open(&mut self, device_name: &str, stream: i32) -> Result<(), PCIe40Error> {
        println!("HERE 1.1");
        let c_name = match CString::new(device_name) {
            Ok(s) => s,
            Err(_) => return Err(PCIe40Error::IdFind),
        };

        println!("HERE 1.2");
        let id = unsafe { bindings::p40_id_find(c_name.as_ptr()) };

        println!("HERE 1.3");
        if id >= 0 {
            self.open_by_id(id, stream).map_err(|error| {println!("HERE IS ERROR!!!"); error} )
        } else {
            Err(PCIe40Error::IdFind)
        }
    }

    pub fn open_by_id(&mut self, id: i32, stream: i32) -> Result<(), PCIe40Error> {
        self.id = id;
        self.stream = stream;

        if self.is_open() {
            self.close();
        }

        unsafe {
            self.id_fd = bindings::p40_id_open(self.id);
            if self.id_fd < 0 {
                self.close();
                return Err(PCIe40Error::IdOpen);
            }

            self.meta_fd = bindings::p40_stream_open(self.id, bindings::P40_DAQ_STREAM_P40_DAQ_STREAM_META);
            if self.meta_fd < 0 {
                self.close();
                return Err(PCIe40Error::MetaStreamOpen);
            }

            self.stream_fd = bindings::p40_stream_open(self.id, self.stream);
            if self.stream_fd < 0 {
                self.close();
                return Err(PCIe40Error::StreamOpen);
            }

            let enabled = bindings::p40_stream_enabled(self.stream_fd);
            if enabled < 0 {
                self.close();
                return Err(PCIe40Error::StreamGetEnabled);
            } else if enabled == 0 {
                self.close();
                return Err(PCIe40Error::StreamEnabled);
            }

            if bindings::p40_stream_lock(self.stream_fd) < 0 {
                self.close();
                return Err(PCIe40Error::StreamLock);
            }

            let size = bindings::p40_stream_get_host_buf_bytes(self.stream_fd);
            if size < 0 {
                self.close();
                return Err(PCIe40Error::StreamSize);
            }

            self.buffer_size = size as usize;
            self.buffer = bindings::p40_stream_map(self.stream_fd);
            if self.buffer.is_null() {
                self.close();
                return Err(PCIe40Error::StreamMap);
            }

            self.ctrl_fd = bindings::p40_ctrl_open(self.id);
            if self.ctrl_fd < 0 {
                self.close();
                return Err(PCIe40Error::CtrlStreamOpen);
            }

            self.update_device_ptr()?;
            self.internal_read_off = self.device_read_off;
            self.requested_size = 0;

            let mut name_buf = [0i8; PCIE40_UNIQUE_NAME_STR_LENGTH];
            if bindings::p40_id_get_name_unique(self.id_fd, name_buf.as_mut_ptr(), name_buf.len()) != 0 {
                self.close();
                return Err(PCIe40Error::GetName);
            }

            // Convert C string to Rust string
            let c_str = std::ffi::CStr::from_ptr(name_buf.as_ptr());
            self.name = c_str.to_string_lossy().into_owned();

            let vers_id = bindings::p40_id_get_source(self.id_fd);
            self.src_id = (vers_id & (SRC_NUM_MASK | SRC_SUBSYSTEM_MASK)) >> SRC_NUM_SHIFT;
            self.block_version = (vers_id & SRC_VERSION_MASK) >> SRC_VERSION_SHIFT;
        }

        Ok(())
    }

    pub fn set_packing_factor(&mut self, packing_factor: i32) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        unsafe {
            let ret_code = bindings::p40_stream_set_meta_packing(self.stream_fd, packing_factor);
            if ret_code != 0 {
                return Err(PCIe40Error::SetMetaPacking);
            }
        }

        Ok(())
    }

    pub fn set_truncation_threshold(&mut self, trunc_thr: i32) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        unsafe {
            let ret_code = bindings::p40_ctrl_set_trunc_thres(self.ctrl_fd, self.stream, trunc_thr);
            if ret_code != 0 {
                return Err(PCIe40Error::InvalidTruncationThreshold);
            }
        }

        Ok(())
    }

    pub fn enable_mfp_stream(&mut self) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        unsafe {
            let meta_mask = bindings::p40_stream_id_to_meta_mask(self.id, self.stream);
            let ret_code = bindings::p40_stream_enable_mask(self.meta_fd, meta_mask);
            if ret_code != 0 {
                return Err(PCIe40Error::MetaStreamEnable);
            }
        }

        Ok(())
    }

    pub fn disable_mfp_stream(&mut self) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        unsafe {
            let meta_mask = bindings::p40_stream_id_to_meta_mask(self.id, self.stream);
            let ret_code = bindings::p40_stream_disable_mask(self.meta_fd, meta_mask);
            if ret_code != 0 {
                return Err(PCIe40Error::MetaStreamDisable);
            }
        }

        Ok(())
    }

    pub fn configure_mfp(&mut self, packing_factor: i32) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        self.enable_mfp_stream()?;
        self.set_packing_factor(packing_factor)?;

        unsafe {
            let trunc_threshold = bindings::p40_ctrl_get_spill_buf_size(self.ctrl_fd, self.stream);
            self.set_truncation_threshold(trunc_threshold)?;
        }

        Ok(())
    }

    pub fn configure_fragment(&mut self, trunc_thr: i32) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        self.disable_mfp_stream()?;
        self.set_truncation_threshold(trunc_thr)?;

        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        unsafe {
            let ret_code = bindings::p40_stream_reset_logic(self.stream_fd);
            if ret_code != 0 {
                return Err(PCIe40Error::LogicReset);
            }
        }

        self.update_device_ptr()?;
        self.internal_read_off = self.device_read_off;
        self.requested_size = 0;
        self.update_usage()?;
        self.next_ev_id = DEFAULT_NEXT_EV_ID;

        Ok(())
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_dmabuf_fd(&self) -> i32 {
        self.dmabuf_fd
    }

    pub fn close(&mut self) {
        unsafe {
            if self.stream_fd >= 0 {
                bindings::p40_stream_unlock(self.stream_fd);
                bindings::p40_stream_close(self.stream_fd, self.buffer);
            }
            if self.id_fd >= 0 {
                bindings::p40_id_close(self.id_fd);
            }
            if self.ctrl_fd >= 0 {
                bindings::p40_ctrl_close(self.ctrl_fd);
            }
            if self.meta_fd >= 0 {
                bindings::p40_stream_close(self.meta_fd, std::ptr::null_mut());
            }
            if self.dmabuf_fd >= 0 {
                libc::close(self.dmabuf_fd);
            }
        }

        // Reset internal status
        self.id = -1;
        self.stream = bindings::P40_DAQ_STREAM_P40_DAQ_STREAM_NULL;
        self.id_fd = -1;
        self.stream_fd = -1;
        self.ctrl_fd = -1;
        self.meta_fd = -1;
        self.dmabuf_fd = -1;
        self.buffer = std::ptr::null_mut();
        self.buffer_size = 0;
        self.internal_read_off = 0;
        self.device_read_off = 0;
        self.available_data = 0;
        self.requested_size = 0;
        self.next_ev_id = DEFAULT_NEXT_EV_ID;
        self.name = String::from("undefined_device");
    }

    pub fn is_open(&self) -> bool {
        self.id_fd >= 0 && self.stream_fd >= 0 && self.meta_fd >= 0 &&
            self.ctrl_fd >= 0 && !self.buffer.is_null()
    }

    pub fn update_device_ptr(&mut self) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        unsafe {
            let device_read_off = bindings::p40_stream_get_host_buf_read_off(self.stream_fd);
            if device_read_off < 0 {
                return Err(PCIe40Error::HostReadOff);
            }

            self.device_read_off = device_read_off as usize;
        }

        Ok(())
    }

    pub fn update_usage(&mut self) -> Result<(), PCIe40Error> {
        unsafe {
            let device_available_data = bindings::p40_stream_get_host_buf_bytes_used(self.stream_fd);
            if device_available_data < 0 {
                return Err(PCIe40Error::HostBytesUsed);
            }
            self.available_data = device_available_data as usize - self.requested_size;
        }
        Ok(())
    }

    pub fn get_buffer_size(&self) -> usize {
        self.buffer_size
    }

    pub fn get_buffer_occupancy(&mut self) -> Result<usize, PCIe40Error> {
        self.update_usage()?;
        Ok(self.available_data + self.requested_size)
    }

    pub fn ack_read(&mut self) -> Result<(), PCIe40Error> {
        if !self.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        unsafe {
            if bindings::p40_stream_free_host_buf_bytes(self.stream_fd, self.requested_size) < 0 {
                return Err(PCIe40Error::HostFreeBuff);
            }
        }

        self.update_device_ptr()?;
        self.internal_read_off = self.device_read_off;
        self.requested_size = 0;
        self.update_usage()?;

        Ok(())
    }

    pub fn cancel_pending(&mut self) {
        self.internal_read_off = self.device_read_off;
        self.available_data += self.requested_size;
        self.requested_size = 0;
    }
}

// MFP Data structure (simplified from C++ MFP)
#[repr(C)]
pub struct MfpHeader {
    pub magic: u16,
    pub packet_size: u32,
    pub n_banks: u16,
    pub ev_id: u32,
    pub src_id: u32,
    pub align: u8,
    pub block_version: u8,
    // In C++ there would be more fields and arrays
}

#[repr(C)]
pub struct Mfp {
    pub header: MfpHeader,
    // In C++ this would be followed by variable-length data
}

impl Mfp {
    pub fn is_valid(&self) -> bool {
        // Check if magic value is correct (example)
        const VALID_MAGIC: u16 = 0x5046; // "MF" in ASCII
        self.header.magic == VALID_MAGIC && self.header.packet_size >= std::mem::size_of::<MfpHeader>() as u32
    }

    pub fn is_end_run(&self) -> bool {
        // Check if this is an end-of-run marker
        const MFP_END_RUN: u32 = 0xFFFFFFFF;
        self.header.ev_id == MFP_END_RUN
    }
}

// Fragment header structure
#[repr(C)]
pub struct PcieFrgHdr {
    pub le_evid: u32,
    // In real code, there would be more fields here
}

// MFP reader that directly uses PCIe40Reader
pub struct PCIe40MfpReader {
    reader: PCIe40Reader,
}

impl PCIe40MfpReader {
    pub fn new() -> Self {
        Self {
            reader: PCIe40Reader::new(),
        }
    }

    pub fn from_id(id: i32, stream: i32, packing_factor: i32) -> Result<Self, PCIe40Error> {
        let mut reader = PCIe40Reader::new();
        reader.open_by_id(id, stream)?;
        reader.configure_mfp(packing_factor)?;

        Ok(Self { reader })
    }

    pub fn from_name(name: &str, stream: i32, packing_factor: i32) -> Result<Self, PCIe40Error> {
        let mut reader = PCIe40Reader::new();
        println!("HERE 1");
        reader.open(name, stream)?;
        println!("HERE 2");
        reader.configure_mfp(packing_factor)?;
        println!("HERE 3");

        Ok(Self { reader })
    }

    pub fn get_name(&self) -> &str {
        self.reader.get_name()
    }

    pub fn get_dmabuf_fd(&self) -> i32 {
        self.reader.get_dmabuf_fd()
    }

    pub fn read_complete(&mut self) -> Result<(), PCIe40Error> {
        self.reader.ack_read()
    }

    pub fn flush(&mut self) -> Result<(), PCIe40Error> {
        self.reader.reset()
    }

    pub fn get_src_id(&self) -> i32 {
        self.reader.src_id
    }

    pub fn get_buffer_size(&self) -> usize {
        self.reader.get_buffer_size()
    }

    pub fn get_buffer_occupancy(&mut self) -> Result<usize, PCIe40Error> {
        self.reader.get_buffer_occupancy()
    }

    // Read data from the device
    pub fn try_get_element(&mut self) -> Result<Option<&Mfp>, PCIe40Error> {
        if !self.reader.is_open() {
            return Err(PCIe40Error::DeviceNotOpen);
        }

        self.reader.update_usage()?;

        // Check if we have enough data for at least an MFP header
        if self.reader.available_data < std::mem::size_of::<MfpHeader>() {
            return Ok(None); // Not enough data yet
        }

        // Get a pointer to the current read position in the buffer
        let mfp_ptr = unsafe {
            (self.reader.buffer as *const u8)
                .add(self.reader.internal_read_off) as *const Mfp
        };

        // Get a reference to the MFP at that position
        let mfp = unsafe { &*mfp_ptr };

        // Validate the MFP
        if !mfp.is_valid() {
            return Err(PCIe40Error::CorruptedData);
        }

        // Calculate total size including alignment padding
        let size = mfp.header.packet_size as usize;
        let aligned_size = align_to(size, 8); // Assuming 8-byte alignment

        // Check if we have the full packet
        if self.reader.available_data < aligned_size {
            return Ok(None); // Not enough data for the full packet
        }

        // Update read offset
        self.update_read_offset(aligned_size)?;

        // Check event ID validity
        let ev_id = mfp.header.ev_id;
        if mfp.is_end_run() {
            self.reader.next_ev_id = 0;
        } else if ev_id != self.reader.next_ev_id {
            return Err(PCIe40Error::CorruptedEvId);
        } else {
            self.reader.next_ev_id += mfp.header.n_banks as u32;
        }

        Ok(Some(mfp))
    }

    // Helper function to update read offset
    fn update_read_offset(&mut self, size: usize) -> Result<(), PCIe40Error> {
        if self.reader.available_data < size {
            return Err(PCIe40Error::CorruptedData);
        }

        self.reader.internal_read_off += size;
        self.reader.requested_size += size;
        self.reader.available_data -= size;

        // Wrap around if needed
        if self.reader.internal_read_off > self.reader.buffer_size {
            self.reader.internal_read_off -= self.reader.buffer_size;
        }

        Ok(())
    }

    // Process data from the device, returning parsed data
    pub fn process_data(&mut self) -> Result<Vec<u8>, PCIe40Error> {
        let mut data = Vec::new();

        // Try to get an element
        match self.try_get_element()? {
            Some(mfp) => {
                // In a real implementation, you'd properly extract data from the MFP
                // For this example, we'll just return the size
                println!("Found MFP with {} banks, size: {} bytes",
                         mfp.header.n_banks, mfp.header.packet_size);

                // Here we would extract actual data from the MFP
                // For simplicity, just creating a sample buffer
                data.resize(mfp.header.packet_size as usize, 0);

                // Acknowledge that we've read this data
                self.read_complete()?;
            },
            None => {
                println!("No data available yet");
            }
        }

        Ok(data)
    }
}

// Default implementation for PCIe40Reader
impl Default for PCIe40Reader {
    fn default() -> Self {
        Self::new()
    }
}

// Default implementation for PCIe40MfpReader
impl Default for PCIe40MfpReader {
    fn default() -> Self {
        Self::new()
    }
}

// Helper function to align sizes
fn align_to(size: usize, alignment: usize) -> usize {
    (size + alignment - 1) & !(alignment - 1)
}
*/

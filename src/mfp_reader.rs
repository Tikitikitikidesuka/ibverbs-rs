use crate::bindings::*;
use std::ffi::{CString, c_int, c_void};
use std::ptr;
use log::{debug, info, warn, error, trace};

// ------------------------------------ //
// -------- TRAIT DEFINITIONS -------- //
// ------------------------------------ //

pub struct MFP {}

pub trait MFPReader {
    type MFPIteratorType: Iterator<Item = Result<Self::ErrorType, MFP>>;
    type ErrorType: std::error::Error;

    fn iter() -> Self::MFPIteratorType;
}

// ------------------------------------ //
// --------- ERROR DEFINITIONS ------- //
// ------------------------------------ //

#[derive(thiserror::Error, Debug)]
pub enum PCIe40ReadError {}

#[derive(thiserror::Error, Debug)]
pub enum PCIe40OpenError {
    #[error("Failed to find device with name \"{device_name}\"")]
    DeviceNotFoundByName { device_name: String },

    #[error("Failed to find device with id {device_id}")]
    DeviceNotFoundById { device_id: i32 },

    #[error("Failed to open device with id {device_id}")]
    DeviceOpenError { device_id: i32 },

    #[error("Failed to gather info from device with id {device_id}")]
    DeviceInfoError {device_id: i32 },

    #[error("Failed to open stream {stream_id} of device {device_id}")]
    StreamOpenError {
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    },

    #[error("Stream {stream_id} of device {device_id} is not enabled")]
    StreamNotEnabled {
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    },

    #[error("Failed to lock stream {stream_id} of device {device_id}")]
    StreamLockError {
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    },

    #[error("Failed to gather info from the buffer of stream {stream_id} of device {device_id}")]
    BufferInfoError {
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    },

    #[error("Failed to map the buffer of stream {stream_id} of device {device_id}")]
    BufferMapError {
        device_id: c_int,
        stream_id: P40_DAQ_STREAM,
    },
}

// ------------------------------------ //
// --------- RESOURCE WRAPPERS ------- //
// ------------------------------------ //

/// Safe wrapper for device ID resource
struct DeviceHandle {
    id: i32,
    fd: c_int,
}

impl DeviceHandle {
    /// Open a device by ID
    fn open(device_id: i32) -> Result<Self, PCIe40OpenError> {
        debug!("Opening device with ID: {}", device_id);

        let fd = unsafe { p40_id_open(device_id) };
        if fd < 0 {
            error!("Failed to open device with ID: {}, fd: {}", device_id, fd);
            return Err(PCIe40OpenError::DeviceOpenError { device_id });
        }

        debug!("Device {} opened successfully with fd: {}", device_id, fd);
        Ok(Self { id: device_id, fd })
    }

    /// Find a device by name and open it
    fn find_and_open(device_name: &str) -> Result<Self, PCIe40OpenError> {
        debug!("Searching for device ID by name: '{}'", device_name);

        // Get name as a C string
        let c_name = match CString::new(device_name) {
            Ok(name) => name,
            Err(e) => {
                error!("Failed to convert device name '{}' to CString: {}", device_name, e);
                return Err(PCIe40OpenError::DeviceNotFoundByName {
                    device_name: device_name.to_string(),
                });
            }
        };

        // Get device id
        let device_id = unsafe { p40_id_find(c_name.as_ptr()) };
        if device_id < 0 {
            error!("Device with name '{}' not found", device_name);
            return Err(PCIe40OpenError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            });
        }

        debug!("Found device ID {} for name '{}'", device_id, device_name);
        Self::open(device_id)
    }

    /// Get the device's unique name
    fn get_unique_name(&self) -> Result<String, PCIe40OpenError> {
        debug!("Getting unique name for device ID: {}, fd: {}", self.id, self.fd);

        let mut name_buf = [0u8; 32];
        let result = unsafe {
            p40_id_get_name_unique(self.fd, name_buf.as_mut_ptr() as *mut i8, name_buf.len())
        };

        if result != 0 {
            error!("Failed to get unique name for device {}, error code: {}", self.id, result);
            return Err(PCIe40OpenError::DeviceInfoError { device_id: self.id });
        }

        // Convert name to string
        let name_end = name_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(name_buf.len());
        let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();

        debug!("Device {} has unique name: '{}'", self.id, name);
        Ok(name)
    }
}

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        debug!("Closing device {}", self.id);
        unsafe { p40_id_close(self.fd) };
    }
}

/// Safe wrapper for stream resource
struct StreamHandle {
    device_id: i32,
    stream_id: P40_DAQ_STREAM,
    fd: c_int,
    is_locked: bool,
}

impl StreamHandle {
    /// Open a stream
    fn open(device_id: i32, stream_id: P40_DAQ_STREAM) -> Result<Self, PCIe40OpenError> {
        debug!("Setting up stream {} for device {}", stream_id, device_id);

        // Open stream
        let fd = unsafe { p40_stream_open(device_id, stream_id) };
        if fd < 0 {
            error!("Failed to open stream {} for device {}, error: {}", stream_id, device_id, fd);
            return Err(PCIe40OpenError::StreamOpenError {
                device_id,
                stream_id,
            });
        }
        debug!("Stream {} opened for device {}", stream_id, device_id);

        // Check if stream is enabled
        let enabled = unsafe { p40_stream_enabled(fd) };
        if enabled != 1 {
            error!("Stream {} of device {} is not enabled, status: {}", stream_id, device_id, enabled);
            unsafe { p40_stream_close(fd, std::ptr::null_mut()) };
            return Err(PCIe40OpenError::StreamNotEnabled {
                device_id,
                stream_id,
            });
        }
        debug!("Stream {} of device {} is enabled", stream_id, device_id);

        Ok(Self {
            device_id,
            stream_id,
            fd,
            is_locked: false,
        })
    }

    /// Lock the stream
    fn lock(&mut self) -> Result<(), PCIe40OpenError> {
        if self.is_locked {
            debug!("Stream {} of device {} already locked", self.stream_id, self.device_id);
            return Ok(());
        }

        let lock_result = unsafe { p40_stream_lock(self.fd) };
        if lock_result != 0 {
            error!("Failed to lock stream {} of device {}, error: {}",
                self.stream_id, self.device_id, lock_result);
            return Err(PCIe40OpenError::StreamLockError {
                device_id: self.device_id,
                stream_id: self.stream_id,
            });
        }

        self.is_locked = true;
        debug!("Stream {} of device {} locked successfully", self.stream_id, self.device_id);
        Ok(())
    }

    /// Unlock the stream
    fn unlock(&mut self) {
        if self.is_locked {
            unsafe { p40_stream_unlock(self.fd) };
            self.is_locked = false;
            debug!("Stream {} of device {} unlocked", self.stream_id, self.device_id);
        }
    }

    /// Get the stream's meta mask
    fn get_meta_mask(&self, device_id: i32) -> i32 {
        unsafe { p40_stream_id_to_meta_mask(device_id, self.fd) }
    }

    /// Set the packing factor
    fn set_packing_factor(&self, factor: u32) -> Result<(), PCIe40OpenError> {
        let result = unsafe { p40_stream_set_meta_packing(self.fd, factor as i32) };
        if result != 0 {
            error!("Failed to set packing factor for device {}, error: {}",
                self.device_id, result);
            return Err(PCIe40OpenError::StreamOpenError {
                device_id: self.device_id,
                stream_id: self.stream_id,
            });
        }
        debug!("Packing factor {} set for stream {} of device {}",
            factor, self.stream_id, self.device_id);
        Ok(())
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        if self.is_locked {
            self.unlock();
        }
        debug!("Closing stream {} of device {}", self.stream_id, self.device_id);
        unsafe { p40_stream_close(self.fd, ptr::null_mut()) };
    }
}

/// Safe wrapper for buffer resource
struct BufferHandle {
    device_id: i32,
    stream_id: P40_DAQ_STREAM,
    stream_fd: c_int,
    buffer: *mut c_void,
    buffer_size: usize,
    read_offset: usize,
}

impl BufferHandle {
    /// Map the buffer
    fn map(stream: &StreamHandle) -> Result<Self, PCIe40OpenError> {
        debug!("Setting up buffer for stream {} of device {}",
            stream.stream_id, stream.device_id);

        // Get buffer size
        let buffer_size = unsafe { p40_stream_get_host_buf_bytes(stream.fd) } as usize;
        if buffer_size <= 0 {
            error!("Failed to get buffer size for stream {} of device {}, size: {}",
                stream.stream_id, stream.device_id, buffer_size);
            return Err(PCIe40OpenError::BufferInfoError {
                device_id: stream.device_id,
                stream_id: stream.stream_id,
            });
        }
        debug!("Buffer size for stream {} of device {}: {}",
            stream.stream_id, stream.device_id, buffer_size);

        // Map buffer
        let buffer = unsafe { p40_stream_map(stream.fd) };
        if buffer.is_null() {
            error!("Failed to map buffer for stream {} of device {}",
                stream.stream_id, stream.device_id);
            return Err(PCIe40OpenError::BufferMapError {
                device_id: stream.device_id,
                stream_id: stream.stream_id,
            });
        }
        debug!("Buffer mapped for stream {} of device {} at address {:p}",
            stream.stream_id, stream.device_id, buffer);

        // Get read offset
        let read_offset = unsafe { p40_stream_get_host_buf_read_off(stream.fd) } as usize;
        if read_offset < 0 {
            error!("Failed to get read offset for stream {} of device {}, offset: {}",
                stream.stream_id, stream.device_id, read_offset);
            unsafe { p40_stream_close(stream.fd, buffer) };
            return Err(PCIe40OpenError::BufferInfoError {
                device_id: stream.device_id,
                stream_id: stream.stream_id,
            });
        }
        debug!("Read offset for stream {} of device {}: {}",
            stream.stream_id, stream.device_id, read_offset);

        trace!("Buffer setup complete: addr={:p}, size={}, read_offset={}",
            buffer, buffer_size, read_offset);

        Ok(Self {
            device_id: stream.device_id,
            stream_id: stream.stream_id,
            stream_fd: stream.fd,
            buffer,
            buffer_size,
            read_offset,
        })
    }
}

// No Drop implementation for BufferHandle because it's handled in StreamHandle's Drop

/// Control stream wrapper
struct ControlHandle {
    device_id: i32,
    fd: c_int,
}

impl ControlHandle {
    /// Open control stream
    fn open(device_id: i32) -> Result<Self, PCIe40OpenError> {
        debug!("Opening control stream for device {}", device_id);

        let fd = unsafe { p40_ctrl_open(device_id) };
        if fd < 0 {
            error!("Failed to open control stream for device {}, error: {}", device_id, fd);
            return Err(PCIe40OpenError::StreamOpenError {
                device_id,
                stream_id: P40_DAQ_STREAM_P40_DAQ_STREAM_META, // Using META as placeholder
            });
        }

        debug!("Control stream opened for device {}", device_id);
        Ok(Self { device_id, fd })
    }

    /// Get spill buffer size
    fn get_spill_buffer_size(&self, stream_fd: c_int) -> i32 {
        unsafe { p40_ctrl_get_spill_buf_size(self.fd, stream_fd) }
    }

    /// Set truncation threshold
    fn set_truncation_threshold(&self, stream_fd: c_int, threshold: i32) -> Result<(), PCIe40OpenError> {
        let result = unsafe { p40_ctrl_set_trunc_thres(self.fd, stream_fd, threshold) };
        if result != 0 {
            error!("Failed to set truncation threshold for device {}, error: {}",
                self.device_id, result);
            return Err(PCIe40OpenError::StreamOpenError {
                device_id: self.device_id,
                stream_id: P40_DAQ_STREAM_P40_DAQ_STREAM_META, // Using META as placeholder
            });
        }

        debug!("Truncation threshold set for device {}", self.device_id);
        Ok(())
    }
}

impl Drop for ControlHandle {
    fn drop(&mut self) {
        debug!("Closing control stream for device {}", self.device_id);
        unsafe { p40_ctrl_close(self.fd) };
    }
}

// ------------------------------------ //
// --------- ITERATOR STRUCTURE ------ //
// ------------------------------------ //

pub struct PCIe40MFPIterator {}

impl Iterator for PCIe40MFPIterator {
    type Item = Result<PCIe40ReadError, MFP>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

// ------------------------------------ //
// --------- READER STRUCTURE -------- //
// ------------------------------------ //

/// PCIe40 MFP Reader implementation
pub struct PCIe40MFPReader {
    device_id: i32,
    device_name: String,
    device_handle: DeviceHandle,
    stream_handle: StreamHandle,
    buffer: BufferHandle,
    internal_read_offset: usize,
    next_ev_id: u32,
}

impl MFPReader for PCIe40MFPReader {
    type MFPIteratorType = PCIe40MFPIterator;
    type ErrorType = PCIe40ReadError;

    fn iter() -> Self::MFPIteratorType {
        trace!("Creating new PCIe40MFPIterator instance");
        todo!()
    }
}

impl PCIe40MFPReader {
    /// Open a PCIe40 device by name
    pub fn open_by_device_name(name: &str, packing_factor: u32) -> Result<Self, PCIe40OpenError> {
        info!("Opening PCIe40 device by name: '{}' with packing factor: {}", name, packing_factor);

        // Find and open device
        let device_handle = DeviceHandle::find_and_open(name)?;

        // Create reader using device ID
        Self::create_reader(device_handle, packing_factor)
    }

    /// Open a PCIe40 device by ID
    pub fn open_by_device_id(id: i32, packing_factor: u32) -> Result<Self, PCIe40OpenError> {
        info!("Opening PCIe40 device by ID: {} with packing factor: {}", id, packing_factor);

        // Open device
        let device_handle = DeviceHandle::open(id)?;

        // Create reader
        Self::create_reader(device_handle, packing_factor)
    }

    /// Common function to create a reader after the device is opened
    fn create_reader(device_handle: DeviceHandle, packing_factor: u32) -> Result<Self, PCIe40OpenError> {
        const MAIN_STREAM: P40_DAQ_STREAM = P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN;

        // Get device name
        let device_name = device_handle.get_unique_name()?;
        let device_id = device_handle.id;

        // Open and setup main stream
        let mut stream_handle = StreamHandle::open(device_id, MAIN_STREAM)?;
        stream_handle.lock()?;

        // Map buffer
        let buffer = BufferHandle::map(&stream_handle)?;

        // Save the read offset before we move the buffer
        let read_offset = buffer.read_offset;

        // Configure MFP mode
        Self::configure_mfp_mode(device_id, &stream_handle, packing_factor)?;

        let reader = PCIe40MFPReader {
            device_id,
            device_name,
            device_handle,
            stream_handle,
            buffer,
            internal_read_offset: read_offset,
            next_ev_id: 0,
        };

        info!("PCIe40MFPReader successfully created for device {}", device_id);
        Ok(reader)
    }

    /// Configure the device for MFP mode
    fn configure_mfp_mode(
        device_id: i32,
        main_stream: &StreamHandle,
        packing_factor: u32
    ) -> Result<(), PCIe40OpenError> {
        debug!("Configuring MFP mode for device {} with packing factor {}", device_id, packing_factor);
        const META_STREAM: P40_DAQ_STREAM = P40_DAQ_STREAM_P40_DAQ_STREAM_META;

        // Open meta stream in its own scope to ensure it's closed when done
        {
            let meta_stream = StreamHandle::open(device_id, META_STREAM)?;

            // Get and enable meta mask
            let meta_mask = main_stream.get_meta_mask(device_id);
            debug!("Meta mask for device {}: {}", device_id, meta_mask);

            let enable_result = unsafe { p40_stream_enable_mask(meta_stream.fd, meta_mask) };
            if enable_result != 0 {
                error!("Failed to enable meta mask for device {}, error: {}", device_id, enable_result);
                return Err(PCIe40OpenError::StreamOpenError {
                    device_id,
                    stream_id: META_STREAM
                });
            }
            debug!("Meta mask enabled for device {}", device_id);
        }

        // Set packing factor
        main_stream.set_packing_factor(packing_factor)?;

        // Configure control settings
        {
            let ctrl = ControlHandle::open(device_id)?;

            // Get and set truncation threshold
            let trunc_thr = ctrl.get_spill_buffer_size(main_stream.fd);
            debug!("Spill buffer size/truncation threshold for device {}: {}", device_id, trunc_thr);

            ctrl.set_truncation_threshold(main_stream.fd, trunc_thr)?;
        }

        info!("MFP mode configured successfully for device {}", device_id);
        Ok(())
    }
}

// Note: No explicit Drop implementation for PCIe40MFPReader needed
// Resource cleanup happens automatically through the Drop implementations
// of the contained handles
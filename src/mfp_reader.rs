/// NEW VERSION
use crate::bindings::*;
use std::ffi::{CString, c_int, c_void};
use std::ptr;
use log::{debug, info, warn, error, trace};

pub struct MFP {}

pub trait MFPReader {
    type MFPIteratorType: Iterator<Item = Result<Self::ErrorType, MFP>>;
    type ErrorType: std::error::Error;

    fn iter() -> Self::MFPIteratorType;
}

// -------------------------------------- //
// ------  PCIE40 IMPLEMENTATION   ------ //
// -------------------------------------- //

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

    #[error("Failed to open stream {device_id} of device {stream_id}")]
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

pub struct PCIe40MFPIterator {}

impl Iterator for PCIe40MFPIterator {
    type Item = Result<PCIe40ReadError, MFP>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

pub struct PCIe40MFPReader {
    device_id: i32,
    device_name: String,
    id_fd: c_int,
    stream_fd: c_int,
    buffer: *mut c_void,
    buffer_size: usize,
    device_read_offset: usize,
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
    pub fn open_by_device_name(name: &str, packing_factor: u32) -> Result<PCIe40MFPReader, PCIe40OpenError> {
        info!("Opening PCIe40 device by name: '{}' with packing factor: {}", name, packing_factor);

        // Get device id from name
        let device_id = Self::get_device_id(name)?;

        // Open device by its id
        Self::open_by_device_id(device_id, packing_factor)
    }

    pub fn open_by_device_id(id: i32, packing_factor: u32) -> Result<PCIe40MFPReader, PCIe40OpenError> {
        info!("Opening PCIe40 device by ID: {} with packing factor: {}", id, packing_factor);
        const MAIN_STREAM: P40_DAQ_STREAM = P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN;

        // Open device
        let id_fd = Self::open_device(id)?;

        // Get device unique name
        let name = Self::get_device_name(id, id_fd)?;

        // Setup stream
        let stream_fd = Self::setup_stream(id_fd, id, MAIN_STREAM)?;

        // Setup buffer
        let (buffer, buffer_size, device_read_offset) =
            Self::setup_buffer(stream_fd, id, MAIN_STREAM)?;

        // Configure MFP mode
        Self::configure_mfp_mode(id, stream_fd, packing_factor)?;

        let reader = PCIe40MFPReader {
            device_id: id,
            device_name: name,
            id_fd,
            stream_fd,
            buffer,
            buffer_size,
            device_read_offset,
            internal_read_offset: device_read_offset,
            next_ev_id: 0,
        };

        info!("PCIe40MFPReader successfully created for device {}", id);
        Ok(reader)
    }

    fn get_device_id(device_name: &str) -> Result<c_int, PCIe40OpenError> {
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
        };

        debug!("Found device ID {} for name '{}'", device_id, device_name);
        Ok(device_id)
    }

    fn open_device(device_id: i32) -> Result<c_int, PCIe40OpenError> {
        debug!("Opening device with ID: {}", device_id);

        // Open device id
        let id_fd = unsafe { p40_id_open(device_id) };
        if id_fd < 0 {
            error!("Failed to open device with ID: {}, fd: {}", device_id, id_fd);
            return Err(PCIe40OpenError::DeviceOpenError { device_id });
        }

        debug!("Device {} opened successfully with fd: {}", device_id, id_fd);
        Ok(id_fd)
    }

    fn get_device_name(device_id: i32, id_fd: c_int) -> Result<String, PCIe40OpenError> {
        debug!("Getting unique name for device ID: {}, fd: {}", device_id, id_fd);

        // Get device name
        let mut name_buf = [0u8; 256];
        let result = unsafe {
            p40_id_get_name_unique(id_fd, name_buf.as_mut_ptr() as *mut i8, name_buf.len())
        };

        if result != 0 {
            error!("Failed to get unique name for device {}, error code: {}", device_id, result);
            unsafe {
                p40_id_close(id_fd);
            }
            return Err(PCIe40OpenError::DeviceInfoError { device_id });
        }

        // Convert name to string
        let name_end = name_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(name_buf.len());
        let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();

        debug!("Device {} has unique name: '{}'", device_id, name);
        Ok(name)
    }

    fn setup_stream(
        id_fd: c_int,
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    ) -> Result<c_int, PCIe40OpenError> {
        debug!("Setting up stream {} for device {}", stream_id, device_id);

        // Open stream
        let stream_fd = unsafe { p40_stream_open(device_id, stream_id) };
        if stream_fd < 0 {
            error!("Failed to open stream {} for device {}, error: {}", stream_id, device_id, stream_fd);
            unsafe { p40_id_close(id_fd) };
            return Err(PCIe40OpenError::StreamOpenError {
                device_id,
                stream_id,
            });
        }
        debug!("Stream {} opened for device {}", stream_id, device_id);

        // Check if stream is enabled
        let enabled = unsafe { p40_stream_enabled(stream_fd) };
        if enabled != 1 {
            error!("Stream {} of device {} is not enabled, status: {}", stream_id, device_id, enabled);
            unsafe {
                p40_stream_close(stream_fd, std::ptr::null_mut());
            }
            return Err(PCIe40OpenError::StreamNotEnabled {
                device_id,
                stream_id,
            });
        }
        debug!("Stream {} of device {} is enabled", stream_id, device_id);

        // Lock stream
        let lock_result = unsafe { p40_stream_lock(stream_fd) };
        if lock_result != 0 {
            error!("Failed to lock stream {} of device {}, error: {}", stream_id, device_id, lock_result);
            unsafe {
                p40_stream_close(stream_fd, std::ptr::null_mut());
                p40_id_close(id_fd);
            }
            return Err(PCIe40OpenError::StreamLockError {
                device_id,
                stream_id,
            });
        }
        debug!("Stream {} of device {} locked successfully", stream_id, device_id);

        Ok(stream_fd)
    }

    fn setup_buffer(
        stream_fd: c_int,
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    ) -> Result<(*mut c_void, usize, usize), PCIe40OpenError> {
        debug!("Setting up buffer for stream {} of device {}", stream_id, device_id);

        // Get buffer size
        let buffer_size = unsafe { p40_stream_get_host_buf_bytes(stream_fd) } as usize;
        if buffer_size <= 0 {
            error!("Failed to get buffer size for stream {} of device {}, size: {}", 
                  stream_id, device_id, buffer_size);
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, std::ptr::null_mut());
            }
            return Err(PCIe40OpenError::BufferInfoError {
                device_id,
                stream_id,
            });
        }
        debug!("Buffer size for stream {} of device {}: {}", stream_id, device_id, buffer_size);

        // Map buffer
        let buffer = unsafe { p40_stream_map(stream_fd) };
        if buffer.is_null() {
            error!("Failed to map buffer for stream {} of device {}", stream_id, device_id);
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, std::ptr::null_mut());
            }
            return Err(PCIe40OpenError::BufferMapError {
                device_id,
                stream_id,
            });
        }
        debug!("Buffer mapped for stream {} of device {} at address {:p}", stream_id, device_id, buffer);

        // Get read offset
        let device_read_offset = unsafe { p40_stream_get_host_buf_read_off(stream_fd) } as usize;
        if device_read_offset < 0 {
            error!("Failed to get read offset for stream {} of device {}, offset: {}", 
                  stream_id, device_id, device_read_offset);
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, std::ptr::null_mut());
            }
            return Err(PCIe40OpenError::BufferInfoError {
                device_id,
                stream_id,
            });
        }
        debug!("Read offset for stream {} of device {}: {}", stream_id, device_id, device_read_offset);

        trace!("Buffer setup complete: addr={:p}, size={}, read_offset={}", 
              buffer, buffer_size, device_read_offset);
        Ok((buffer, buffer_size, device_read_offset))
    }

    fn configure_mfp_mode(device_id: i32, main_stream_fd: c_int, packing_factor: u32) -> Result<(), PCIe40OpenError> {
        debug!("Configuring MFP mode for device {} with packing factor {}", device_id, packing_factor);
        const META_STREAM: P40_DAQ_STREAM = P40_DAQ_STREAM_P40_DAQ_STREAM_META;

        // Open meta stream
        debug!("Opening meta stream for device {}", device_id);
        let meta_stream_fd = unsafe { p40_stream_open(device_id, META_STREAM) };
        if meta_stream_fd < 0 {
            error!("Failed to open meta stream for device {}, error: {}", device_id, meta_stream_fd);
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }
        debug!("Meta stream opened for device {}", device_id);

        // Get meta mask
        debug!("Getting meta mask for main stream of device {}", device_id);
        let meta_mask = unsafe { p40_stream_id_to_meta_mask(device_id, main_stream_fd) };
        debug!("Meta mask for device {}: {}", device_id, meta_mask);

        // Enable meta mask
        debug!("Enabling meta mask for device {}", device_id);
        let enable_result = unsafe { p40_stream_enable_mask(meta_stream_fd, meta_mask) };
        if enable_result != 0 {
            error!("Failed to enable meta mask for device {}, error: {}", device_id, enable_result);
            unsafe { p40_stream_close(meta_stream_fd, std::ptr::null_mut()) };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }
        debug!("Meta mask enabled for device {}", device_id);

        // Set packing factor
        debug!("Setting packing factor {} for device {}", packing_factor, device_id);
        let packing_result = unsafe { p40_stream_set_meta_packing(main_stream_fd, packing_factor as i32) };
        if packing_result != 0 {
            error!("Failed to set packing factor for device {}, error: {}", device_id, packing_result);
            unsafe { p40_stream_close(meta_stream_fd, std::ptr::null_mut()) };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }
        debug!("Packing factor set for device {}", device_id);

        // Open control stream
        debug!("Opening control stream for device {}", device_id);
        let ctrl_fd = unsafe { p40_ctrl_open(device_id) };
        if ctrl_fd < 0 {
            error!("Failed to open control stream for device {}, error: {}", device_id, ctrl_fd);
            unsafe { p40_stream_close(meta_stream_fd, ptr::null_mut()) };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }
        debug!("Control stream opened for device {}", device_id);

        // Get spill buffer size as truncation threshold
        debug!("Getting spill buffer size for device {}", device_id);
        let trunc_thr = unsafe { p40_ctrl_get_spill_buf_size(ctrl_fd, main_stream_fd) };
        debug!("Spill buffer size/truncation threshold for device {}: {}", device_id, trunc_thr);

        // Set truncation threshold
        debug!("Setting truncation threshold for device {}", device_id);
        let trunc_result = unsafe { p40_ctrl_set_trunc_thres(ctrl_fd, main_stream_fd, trunc_thr) };
        if trunc_result != 0 {
            error!("Failed to set truncation threshold for device {}, error: {}", device_id, trunc_result);
            unsafe {
                p40_ctrl_close(ctrl_fd);
                p40_stream_close(meta_stream_fd, ptr::null_mut());
            };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }
        debug!("Truncation threshold set for device {}", device_id);

        // Close the streams we don't need to keep open
        debug!("Closing temporary streams for device {}", device_id);
        unsafe {
            p40_ctrl_close(ctrl_fd);
            p40_stream_close(meta_stream_fd, ptr::null_mut());
        }
        debug!("Temporary streams closed for device {}", device_id);

        info!("MFP mode configured successfully for device {}", device_id);
        Ok(())
    }
}

// Automatic resource cleanup
impl Drop for PCIe40MFPReader {
    fn drop(&mut self) {
        unsafe {
            p40_stream_unlock(self.stream_fd);
            p40_stream_close(self.stream_fd, self.buffer);
            p40_id_close(self.id_fd);
        }
    }
}
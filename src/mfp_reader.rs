use crate::bindings::*;
use std::ffi::{CString, c_int, c_void};
use std::ptr;

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
        todo!()
    }
}

impl PCIe40MFPReader {
    pub fn open_by_device_name(name: &str, packing_factor: u32) -> Result<PCIe40MFPReader, PCIe40OpenError> {
        // Get device id from name
        let device_id = Self::get_device_id(name)?;

        // Open device by its id
        Self::open_by_device_id(device_id, packing_factor)
    }

    pub fn open_by_device_id(id: i32, packing_factor: u32) -> Result<PCIe40MFPReader, PCIe40OpenError> {
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
        Self::configure_mfp_mode(stream_fd, id, packing_factor)?;

        Ok(PCIe40MFPReader {
            device_id: id,
            device_name: name,
            id_fd,
            stream_fd,
            buffer,
            buffer_size,
            device_read_offset,
            internal_read_offset: device_read_offset,
            next_ev_id: 0,
        })
    }

    fn get_device_id(device_name: &str) -> Result<c_int, PCIe40OpenError> {
        // Get name as a C string
        let c_name = CString::new(device_name).map_err(|_| PCIe40OpenError::DeviceNotFoundByName {
            device_name: device_name.to_string(),
        })?;

        // Get device id
        let device_id = unsafe { p40_id_find(c_name.as_ptr()) };
        if device_id < 0 {
            return Err(PCIe40OpenError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            });
        };

        Ok(device_id)
    }

    fn open_device(device_id: i32) -> Result<c_int, PCIe40OpenError> {
        // Open device id
        let id_fd = unsafe { p40_id_open(device_id) };
        if id_fd < 0 {
            return Err(PCIe40OpenError::DeviceOpenError { device_id });
        }

        Ok(id_fd)
    }

    fn get_device_name(device_id: i32, id_fd: c_int) -> Result<String, PCIe40OpenError> {
        // Get device name
        let mut name_buf = [0u8; 256];
        if unsafe {
            p40_id_get_name_unique(id_fd, name_buf.as_mut_ptr() as *mut i8, name_buf.len())
        } != 0
        {
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

        Ok(name)
    }

    fn setup_stream(
        id_fd: c_int,
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    ) -> Result<c_int, PCIe40OpenError> {
        // Open stream
        let stream_fd = unsafe { p40_stream_open(id_fd, stream_id) };
        if stream_fd != 0 {
            unsafe { p40_id_close(id_fd) };
            return Err(PCIe40OpenError::StreamOpenError {
                device_id,
                stream_id,
            });
        }

        // Check if stream is enabled
        let enabled = unsafe { p40_stream_enabled(stream_fd) };
        if enabled != 0 {
            unsafe {
                p40_stream_close(stream_fd, std::ptr::null_mut());
            }
            return Err(PCIe40OpenError::StreamNotEnabled {
                device_id,
                stream_id,
            });
        }

        // Lock stream
        if unsafe { p40_stream_lock(stream_fd) } != 0 {
            unsafe {
                p40_stream_close(stream_fd, std::ptr::null_mut());
                p40_id_close(id_fd);
            }
            return Err(PCIe40OpenError::StreamLockError {
                device_id,
                stream_id,
            });
        }

        Ok(stream_fd)
    }

    fn setup_buffer(
        stream_fd: c_int,
        device_id: i32,
        stream_id: P40_DAQ_STREAM,
    ) -> Result<(*mut c_void, usize, usize), PCIe40OpenError> {
        // Get buffer size
        let buffer_size = unsafe { p40_stream_get_host_buf_bytes(stream_fd) } as usize;
        if buffer_size != 0 {
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, std::ptr::null_mut());
                // Note: id_fd is no longer in scope here
            }
            return Err(PCIe40OpenError::BufferInfoError {
                device_id,
                stream_id,
            });
        }

        // Map buffer
        let buffer = unsafe { p40_stream_map(stream_fd) };
        if buffer.is_null() {
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, std::ptr::null_mut());
                // Note: id_fd is no longer in scope here
            }
            return Err(PCIe40OpenError::BufferMapError {
                device_id,
                stream_id,
            });
        }

        // Get read offset
        let device_read_offset = unsafe { p40_stream_get_host_buf_read_off(stream_fd) } as usize;
        if device_read_offset != 0 {
            unsafe {
                p40_stream_unlock(stream_fd);
                p40_stream_close(stream_fd, std::ptr::null_mut());
                // Note: id_fd is no longer in scope here
            }
            return Err(PCIe40OpenError::BufferInfoError {
                device_id,
                stream_id,
            });
        }

        Ok((buffer, buffer_size, device_read_offset))
    }

    fn configure_mfp_mode(device_id: i32, main_stream_fd: c_int, packing_factor: u32) -> Result<(), PCIe40OpenError> {
        const META_STREAM: P40_DAQ_STREAM = P40_DAQ_STREAM_P40_DAQ_STREAM_META;

        // Open meta stream
        let meta_stream_fd = unsafe { p40_stream_open(device_id, META_STREAM) };
        if meta_stream_fd != 0 {
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }

        // Get meta mask
        let meta_mask = unsafe { p40_stream_id_to_meta_mask(device_id, main_stream_fd) };

        // Enable meta mask
        if unsafe { p40_stream_enable_mask(meta_stream_fd, meta_mask) } != 0 {
            unsafe { p40_stream_close(meta_stream_fd, std::ptr::null_mut()) };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }

        // Set packing factor
        if unsafe { p40_stream_set_meta_packing(main_stream_fd, packing_factor as i32) } != 0 {
            unsafe { p40_stream_close(meta_stream_fd, std::ptr::null_mut()) };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }

        // Open control stream
        let ctrl_fd = unsafe { p40_ctrl_open(device_id) };
        if ctrl_fd != 0 {
            unsafe { p40_stream_close(meta_stream_fd, ptr::null_mut()) };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }

        // Get spill buffer size as truncation threshold
        let trunc_thr = unsafe { p40_ctrl_get_spill_buf_size(ctrl_fd, main_stream_fd) };

        // Set truncation threshold
        if unsafe { p40_ctrl_set_trunc_thres(ctrl_fd, main_stream_fd, trunc_thr) } != 0 {
            unsafe {
                p40_ctrl_close(ctrl_fd);
                p40_stream_close(meta_stream_fd, ptr::null_mut());
            };
            return Err(PCIe40OpenError::StreamOpenError {device_id, stream_id: META_STREAM});
        }

        // Close the streams we don't need to keep open
        unsafe {
            p40_ctrl_close(ctrl_fd);
            p40_stream_close(meta_stream_fd, ptr::null_mut());
        }

        Ok(())
    }
}

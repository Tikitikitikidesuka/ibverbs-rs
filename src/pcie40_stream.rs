use crate::bindings::*;
use crate::pcie40_id::{PCIe40IdManager, PCIe40IdManagerError};
use std::ptr::slice_from_raw_parts;
use std::slice;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PCIe40DAQStreamType {
    MainStream,
    Odin0Stream,
    Odin1Stream,
    Odin2Stream,
    Odin3Stream,
    Odin4Stream,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PCIe40DAQStreamFormat {
    RawFormat,
    MetaFormat,
}

impl From<PCIe40DAQStreamType> for P40_DAQ_STREAM {
    fn from(value: PCIe40DAQStreamType) -> Self {
        match value {
            PCIe40DAQStreamType::MainStream => P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN,
            PCIe40DAQStreamType::Odin0Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN0,
            PCIe40DAQStreamType::Odin1Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN1,
            PCIe40DAQStreamType::Odin2Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN2,
            PCIe40DAQStreamType::Odin3Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN3,
            PCIe40DAQStreamType::Odin4Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN4,
        }
    }
}

pub struct PCIe40StreamManager {}

#[derive(Debug, Error)]
pub enum PCIe40StreamManagerError {
    #[error("Invalid device name: {device_name}")]
    InvalidDeviceName { device_name: String },

    #[error("Error reading from PCIe40 drivers")]
    DriverReadError,

    #[error("Device with name \"{device_name}\" not found")]
    DeviceNotFoundByName { device_name: String },

    #[error("Device with id {device_id} not found")]
    DeviceNotFoundById { device_id: i32 },

    #[error("Failed to open PCIE40 device with id {device_id}")]
    StreamOpenFail {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },
}

impl PCIe40StreamManager {
    pub fn stream_exists(
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    ) -> Result<bool, PCIe40StreamManagerError> {
        let c_result = unsafe { p40_stream_exists(device_id, stream_type.into()) };

        if c_result < 0 {
            Err(PCIe40StreamManagerError::DriverReadError)
        } else if c_result == 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn open_by_device_name(
        device_name: &str,
        stream_type: PCIe40DAQStreamType,
        stream_format: PCIe40DAQStreamFormat,
    ) -> Result<PCIe40Stream, PCIe40StreamManagerError> {
        let device_id = PCIe40IdManager::find_id_by_name(device_name).or(Err(
            PCIe40StreamManagerError::DeviceNotFoundByName {
                device_name: device_name.into(),
            },
        ))?;

        Self::open_by_device_id(device_id, stream_type, stream_format)
    }

    pub fn open_by_device_id(
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
        stream_format: PCIe40DAQStreamFormat,
    ) -> Result<PCIe40Stream, PCIe40StreamManagerError> {
        if !Self::stream_exists(device_id, stream_type)? {
            Err(PCIe40StreamManagerError::DeviceNotFoundById { device_id })?;
        }

        let stream_fd = unsafe { p40_stream_open(device_id, stream_type.into()) };
        if stream_fd < 0 {
            Err(PCIe40StreamManagerError::StreamOpenFail {
                device_id,
                stream_type,
            })?;
        }

        let meta_stream_fd = unsafe { p40_stream_open(device_id, stream_type.into()) };
        if stream_fd < 0 {
            Err(PCIe40StreamManagerError::StreamOpenFail {
                device_id,
                stream_type,
            })?;
        }

        Ok(PCIe40Stream::new(
            device_id,
            stream_fd,
            meta_stream_fd,
            stream_type,
            stream_format,
        ))
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(stream_endpoint: &mut PCIe40Stream) {
        unsafe {
            p40_ctrl_close(stream_endpoint.stream_fd);
        }
    }
}

pub struct PCIe40Stream {
    device_id: i32,
    stream_fd: i32,
    meta_stream_fd: i32,
    stream_type: PCIe40DAQStreamType,
    stream_format: PCIe40DAQStreamFormat,
    enable_state_action_on_close: PCIe40StreamHandleEnableStateActionOnClose,
}

#[derive(Debug, Error)]
pub enum PCIe40StreamError {
    #[error("Error reading data from the PCIe40 device with id {device_id}")]
    StreamReadError {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Error write data to the PCIe40 device with id {device_id}")]
    StreamWriteError {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to enable stream {stream_type:?} on device {device_id}")]
    FailedToEnableStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to disable stream {stream_type:?} on device {device_id}")]
    FailedToDisableStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to lock stream {stream_type:?} on device {device_id}")]
    FailedToLockStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to unlock stream {stream_type:?} on device {device_id}")]
    FailedToUnlockStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to map buffer of stream {stream_type:?} on device {device_id}")]
    FailedToMapBuffer {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to unmap buffer of stream {stream_type:?} on device {device_id}")]
    FailedToUnmapBuffer {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PCIe40StreamHandleEnableStateCloseMode {
    PreserveEnableState,
    DisabledOnClose,
    EnabledOnClose,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PCIe40StreamHandleEnableStateActionOnClose {
    PreserveEnableState { enabled: bool },
    DisableOnClose,
    EnableOnClose,
}

impl Drop for PCIe40Stream {
    fn drop(&mut self) {
        self.run_raii_enable_state_action();
        PCIe40StreamManager::close(self);
    }
}

impl PCIe40Stream {
    fn new(
        device_id: i32,
        stream_fd: i32,
        meta_stream_fd: i32,
        stream_type: PCIe40DAQStreamType,
        stream_format: PCIe40DAQStreamFormat,
    ) -> Self {
        Self {
            device_id,
            stream_fd,
            meta_stream_fd,
            stream_type,
            stream_format,
            enable_state_action_on_close:
                PCIe40StreamHandleEnableStateActionOnClose::DisableOnClose,
        }
    }

    pub fn enabled(&self) -> Result<bool, PCIe40StreamError> {
        let c_result = unsafe { p40_stream_enabled(self.stream_fd) };

        if c_result < 0 {
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })
        } else {
            Ok(c_result != 0)
        }
    }

    pub fn set_raii_enable_state_close_mode(
        &mut self,
        preserve_mode: PCIe40StreamHandleEnableStateCloseMode,
    ) -> Result<(), PCIe40StreamError> {
        self.enable_state_action_on_close = match preserve_mode {
            PCIe40StreamHandleEnableStateCloseMode::DisabledOnClose => {
                PCIe40StreamHandleEnableStateActionOnClose::DisableOnClose
            }
            PCIe40StreamHandleEnableStateCloseMode::EnabledOnClose => {
                PCIe40StreamHandleEnableStateActionOnClose::EnableOnClose
            }
            PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState => {
                PCIe40StreamHandleEnableStateActionOnClose::PreserveEnableState {
                    enabled: self.enabled()?,
                }
            }
        };

        Ok(())
    }

    pub fn locking_process(&self) -> Result<Option<i32>, PCIe40StreamError> {
        let c_result = unsafe { p40_stream_get_locker(self.stream_fd) };

        if c_result == 0 {
            Ok(None)
        } else if c_result > 0 {
            Ok(Some(c_result))
        } else {
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })
        }
    }

    pub fn lock(&mut self) -> Result<PCIe40StreamGuard, PCIe40StreamError> {
        self.enable()?;

        let c_result = unsafe { p40_stream_lock(self.stream_fd) };

        if c_result == 0 {
            Ok(PCIe40StreamGuard {
                stream_handle: self,
            })
        } else if c_result > 0 {
            Err(PCIe40StreamError::FailedToLockStream {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })
        } else {
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })
        }
    }

    fn enable(&mut self) -> Result<(), PCIe40StreamError> {
        if self.enabled()? || unsafe { p40_stream_enable(self.stream_fd) } == 0 {
            Ok(())
        } else {
            Err(PCIe40StreamError::FailedToEnableStream {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })
        }
    }

    fn disable(&mut self) -> Result<(), PCIe40StreamError> {
        if self.enabled()? && unsafe { p40_stream_disable(self.stream_fd) } != 0 {
            Err(PCIe40StreamError::FailedToDisableStream {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })?
        } else {
            Ok(())
        }
    }

    fn run_raii_enable_state_action(&mut self) {
        match self.enable_state_action_on_close {
            PCIe40StreamHandleEnableStateActionOnClose::DisableOnClose => {
                let _ = self.disable();
            }
            PCIe40StreamHandleEnableStateActionOnClose::EnableOnClose => {
                /* Do nothing... already enabled */
            }
            PCIe40StreamHandleEnableStateActionOnClose::PreserveEnableState { enabled } => {
                if !enabled {
                    let _ = self.disable();
                }
            }
        }
    }
}

pub struct PCIe40StreamGuard<'a> {
    stream_handle: &'a mut PCIe40Stream,
}

impl<'a> Drop for PCIe40StreamGuard<'a> {
    fn drop(&mut self) {
        let _ = self.ref_unlock();
    }
}

impl<'a> PCIe40StreamGuard<'a> {
    fn new(stream_handle: &'a mut PCIe40Stream) -> Result<Self, PCIe40StreamError> {
        let mut locked_stream = PCIe40StreamGuard { stream_handle };

        match locked_stream.stream_handle.stream_format {
            PCIe40DAQStreamFormat::RawFormat => locked_stream.set_meta_enabled_state(false)?,
            PCIe40DAQStreamFormat::MetaFormat => locked_stream.set_meta_enabled_state(true)?,
        };

        Ok(locked_stream)
    }

    fn set_meta_enabled_state(&mut self, enabled: bool) -> Result<(), PCIe40StreamError> {
        let meta_mask = unsafe {
            p40_stream_id_to_meta_mask(
                self.stream_handle.device_id,
                self.stream_handle.stream_type.into(),
            )
        };

        let c_result = match enabled {
            true => unsafe { p40_stream_enable_mask(self.stream_handle.meta_stream_fd, meta_mask) },
            false => unsafe {
                p40_stream_disable_mask(self.stream_handle.meta_stream_fd, meta_mask)
            },
        };

        if c_result == 0 {
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            })
        } else {
            Ok(())
        }
    }

    fn ref_unlock(&mut self) -> Result<(), PCIe40StreamError> {
        let c_result = unsafe { p40_stream_unlock(self.stream_handle.stream_fd) };

        if c_result == 0 {
            Ok(())
        } else if c_result > 0 {
            Err(PCIe40StreamError::FailedToUnlockStream {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            })
        } else {
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            })
        }
    }
}

impl<'a> PCIe40StreamGuard<'a> {
    pub fn map_buffer<'buf>(&'buf mut self) -> Result<PCIe40MappedBuffer<'a, 'buf>, PCIe40StreamError> {
        let buff_ptr = unsafe { p40_stream_map(self.stream_handle.stream_fd) };
        if buff_ptr.is_null() {
            Err(PCIe40StreamError::FailedToMapBuffer {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            })?
        }

        let buff_size = unsafe { p40_stream_get_host_buf_bytes(self.stream_handle.stream_fd) };
        if buff_size <= 0 {
            Err(PCIe40StreamError::FailedToMapBuffer {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            })?
        }

        Ok(PCIe40MappedBuffer::new(self, unsafe {
            slice::from_raw_parts(buff_ptr as *const u8, buff_size as usize)
        }))
    }
}

pub struct PCIe40MappedBuffer<'guard, 'buf> {
    stream_guard: &'buf mut PCIe40StreamGuard<'guard>,
    buffer: &'buf [u8],
}

impl<'guard, 'buf> Drop for PCIe40MappedBuffer<'guard, 'buf> {
    fn drop(&mut self) {
        self.unmap_buffer();
    }
}

impl<'guard, 'buf> PCIe40MappedBuffer<'guard, 'buf> {
    fn new(stream_guard: &'buf mut PCIe40StreamGuard<'guard>, buffer: &'buf [u8]) -> Self {
        Self {
            stream_guard,
            buffer,
        }
    }

    fn unmap_buffer(&mut self) {
        unsafe {
            p40_stream_unmap(
                self.stream_guard.stream_handle.stream_fd,
                self.buffer.as_ptr() as *mut std::os::raw::c_void,
            )
        }
    }
}

impl<'guard, 'buf> PCIe40MappedBuffer<'guard, 'buf> {
    pub fn buffer(&self) -> &[u8] {
        self.buffer
    }
}

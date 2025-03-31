use crate::bindings::*;
use crate::pcie40_id::{PCIe40IdManager, PCIe40IdManagerError};
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
        let c_result = unsafe { p40_stream_exists(device_id, stream_type.into()) };

        if c_result < 0 {
            Err(PCIe40StreamManagerError::DriverReadError)?;
        } else if c_result != 0 {
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

    #[error("Failed to lock the stream")]
    FailedToLockStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to unlock the stream")]
    FailedToUnlockStream {
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

    pub fn enable(mut self) -> Result<PCIe40EnabledStream, PCIe40StreamError> {
        if self.enabled()? || unsafe { p40_stream_enable(self.stream_fd) } == 0 {
            Ok(PCIe40EnabledStream {
                stream_handle: self,
            })
        } else {
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })
        }
    }

    fn disable(&mut self) -> Result<(), PCIe40StreamError> {
        if self.enabled()? && unsafe { p40_stream_disable(self.stream_fd) } != 0 {
            Err(PCIe40StreamError::StreamWriteError {
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

// This type just exists to match the C drivers functions
// In case functionality is added to an enabled yet unlocked stream
// At the moment it does nothing more than act as a transition
// between a stream handle and a locked stream
pub struct PCIe40EnabledStream {
    stream_handle: PCIe40Stream,
}

/*
// No need to implement drop since internal attributes will be dropped automatically
// and this struct does nothing on close, only return ownership of the handle.
impl Drop for PCIe40EnabledStream {
    fn drop(&mut self) {
    }
}
*/

impl PCIe40EnabledStream {
    pub fn close(self) -> PCIe40Stream {
        // Nothing to do here...
        // The disable or not logic is in the handle
        self.stream_handle
    }

    pub fn locking_process(&self) -> Result<Option<i32>, PCIe40StreamError> {
        let c_result = unsafe { p40_stream_get_locker(self.stream_handle.stream_fd) };

        if c_result == 0 {
            Ok(None)
        } else if c_result > 0 {
            Ok(Some(c_result))
        } else {
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            })
        }
    }

    pub fn lock(self) -> Result<PCIe40LockedStream, PCIe40StreamError> {
        let c_result = unsafe { p40_stream_lock(self.stream_handle.stream_fd) };

        if c_result == 0 {
            Ok(PCIe40LockedStream {
                stream_handle: self.stream_handle,
            })
        } else if c_result > 0 {
            Err(PCIe40StreamError::FailedToLockStream {
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

pub struct PCIe40LockedStream {
    stream_handle: PCIe40Stream,
}

impl Drop for PCIe40LockedStream {
    fn drop(&mut self) {
        let _ = self.ref_unlock();
    }
}

impl PCIe40LockedStream {
    pub fn unlock(mut self) -> Result<PCIe40EnabledStream, PCIe40StreamError> {
        self.ref_unlock()?;
        Ok(PCIe40EnabledStream {
            stream_handle: self.stream_handle,
        })
    }

    fn new(stream_handle: PCIe40Stream) -> Result<Self, PCIe40StreamError> {
        let mut locked_stream = PCIe40LockedStream { stream_handle };

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
            false => unsafe { p40_stream_disable_mask(self.stream_handle.meta_stream_fd, meta_mask) },
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

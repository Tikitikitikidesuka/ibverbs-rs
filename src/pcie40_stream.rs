use crate::bindings::*;
use crate::pcie40_id::PCIe40IdManager;
use log::{debug, error, info, trace};
use std::fmt::{Display, Formatter};
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

impl Display for PCIe40DAQStreamType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PCIe40DAQStreamType::MainStream => write!(f, "MAIN"),
            PCIe40DAQStreamType::Odin0Stream => write!(f, "ODIN0"),
            PCIe40DAQStreamType::Odin1Stream => write!(f, "ODIN1"),
            PCIe40DAQStreamType::Odin2Stream => write!(f, "ODIN2"),
            PCIe40DAQStreamType::Odin3Stream => write!(f, "ODIN3"),
            PCIe40DAQStreamType::Odin4Stream => write!(f, "ODIN4"),
        }
    }
}

impl Display for PCIe40DAQStreamFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PCIe40DAQStreamFormat::RawFormat => write!(f, "RAW"),
            PCIe40DAQStreamFormat::MetaFormat => write!(f, "META"),
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
        debug!(
            "Checking if stream {} exists for device {}",
            stream_type, device_id
        );

        trace!("Calling p40_stream_exists({}, {:?})", device_id, stream_type);
        let c_result = unsafe { p40_stream_exists(device_id, stream_type.into()) };
        trace!("p40_stream_exists returned {}", c_result);

        if c_result < 0 {
            error!(
                "Driver read error while checking if stream {} exists for device {}",
                stream_type, device_id
            );
            Err(PCIe40StreamManagerError::DriverReadError)
        } else if c_result == 0 {
            debug!("Stream {} exists for device {}", stream_type, device_id);
            Ok(true)
        } else {
            debug!(
                "Stream {} does not exist for device {}",
                stream_type, device_id
            );
            Ok(false)
        }
    }

    pub fn open_by_device_name(
        device_name: &str,
        stream_type: PCIe40DAQStreamType,
        stream_format: PCIe40DAQStreamFormat,
    ) -> Result<PCIe40Stream, PCIe40StreamManagerError> {
        info!(
            "Opening stream {} with format {} by device name '{}'",
            stream_type, stream_format, device_name
        );

        trace!("Calling PCIe40IdManager::find_id_by_name(\"{}\")", device_name);
        let device_id = PCIe40IdManager::find_id_by_name(device_name).or_else(|_| {
            error!("Device with name '{}' not found", device_name);
            Err(PCIe40StreamManagerError::DeviceNotFoundByName {
                device_name: device_name.into(),
            })
        })?;
        trace!("PCIe40IdManager::find_id_by_name returned {}", device_id);

        Self::open_by_device_id(device_id, stream_type, stream_format)
    }

    pub fn open_by_device_id(
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
        stream_format: PCIe40DAQStreamFormat,
    ) -> Result<PCIe40Stream, PCIe40StreamManagerError> {
        info!(
            "Opening stream {} with format {} by device ID {}",
            stream_type, stream_format, device_id
        );

        if !Self::stream_exists(device_id, stream_type)? {
            error!("Device with id {} not found", device_id);
            return Err(PCIe40StreamManagerError::DeviceNotFoundById { device_id });
        }

        trace!("Calling p40_stream_open({}, {:?})", device_id, stream_type);
        let stream_fd = unsafe { p40_stream_open(device_id, stream_type.into()) };
        trace!("p40_stream_open returned {}", stream_fd);

        if stream_fd < 0 {
            error!(
                "Failed to open stream {} for device {}",
                stream_type, device_id
            );
            return Err(PCIe40StreamManagerError::StreamOpenFail {
                device_id,
                stream_type,
            });
        }
        debug!("Opened {} stream with fd {}", stream_type, stream_fd);

        trace!("Calling p40_stream_open({}, {:?}) for meta stream", device_id, stream_type);
        let meta_stream_fd = unsafe { p40_stream_open(device_id, stream_type.into()) };
        trace!("p40_stream_open for meta stream returned {}", meta_stream_fd);

        if meta_stream_fd < 0 {
            error!(
                "Failed to open meta stream {} for device {}",
                stream_type, device_id
            );
            return Err(PCIe40StreamManagerError::StreamOpenFail {
                device_id,
                stream_type,
            });
        }
        debug!("Opened META stream with fd {}", meta_stream_fd);

        let stream = PCIe40Stream::new(
            device_id,
            stream_fd,
            meta_stream_fd,
            stream_type,
            stream_format,
        );
        info!(
            "Successfully opened stream {} for device {}",
            stream_type, device_id
        );
        Ok(stream)
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(stream_endpoint: &mut PCIe40Stream) {
        debug!(
            "Closing stream {} for device {}",
            stream_endpoint.stream_type, stream_endpoint.device_id
        );

        trace!("Calling p40_ctrl_close({})", stream_endpoint.stream_fd);
        unsafe {
            p40_ctrl_close(stream_endpoint.stream_fd);
        }
        debug!("Closed stream fd {}", stream_endpoint.stream_fd);
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
    #[error("Error reading data from the stream {stream_type} on device {device_id}")]
    StreamReadError {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
        info: String,
    },

    #[error("Error writing data into the stream {stream_type} on device {device_id}")]
    StreamWriteError {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
        info: String,
    },

    #[error("Failed to enable stream {stream_type} on device {device_id}")]
    FailedToEnableStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to disable stream {stream_type} on device {device_id}")]
    FailedToDisableStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to lock stream {stream_type} on device {device_id}")]
    FailedToLockStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to unlock stream {stream_type} on device {device_id}")]
    FailedToUnlockStream {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to map buffer of stream {stream_type} on device {device_id}")]
    FailedToMapBuffer {
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    },

    #[error("Failed to unmap buffer of stream {stream_type} on device {device_id}")]
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
        trace!(
            "Drop called on PCIe40Stream for device {} stream {}",
            self.device_id, self.stream_type
        );
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
        debug!(
            "Creating new PCIe40Stream for device {} with stream fd {} and meta stream fd {}",
            device_id, stream_fd, meta_stream_fd
        );
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
        trace!(
            "Checking if stream {} on device {} is enabled",
            self.stream_type, self.device_id
        );

        trace!("Calling p40_stream_enabled({})", self.stream_fd);
        let c_result = unsafe { p40_stream_enabled(self.stream_fd) };
        trace!("p40_stream_enabled returned {}", c_result);

        if c_result < 0 {
            error!(
                "Unable to read enabled status for stream {} on device {}",
                self.stream_type, self.device_id
            );
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.device_id,
                stream_type: self.stream_type,
                info: "Unable to read enabled status".to_string(),
            })
        } else {
            let is_enabled = c_result != 0;
            trace!(
                "Stream {} on device {} is {}",
                self.stream_type,
                self.device_id,
                if is_enabled { "enabled" } else { "disabled" }
            );
            Ok(is_enabled)
        }
    }

    pub fn set_raii_enable_state_close_mode(
        &mut self,
        preserve_mode: PCIe40StreamHandleEnableStateCloseMode,
    ) -> Result<(), PCIe40StreamError> {
        debug!(
            "Setting RAII enable state close mode to {:?} for stream {} on device {}",
            preserve_mode, self.stream_type, self.device_id
        );
        self.enable_state_action_on_close = match preserve_mode {
            PCIe40StreamHandleEnableStateCloseMode::DisabledOnClose => {
                debug!("Stream will be disabled on close");
                PCIe40StreamHandleEnableStateActionOnClose::DisableOnClose
            }
            PCIe40StreamHandleEnableStateCloseMode::EnabledOnClose => {
                debug!("Stream will be enabled on close");
                PCIe40StreamHandleEnableStateActionOnClose::EnableOnClose
            }
            PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState => {
                let enabled = self.enabled()?;
                debug!(
                    "Stream enable state ({}) will be preserved on close",
                    enabled
                );
                PCIe40StreamHandleEnableStateActionOnClose::PreserveEnableState { enabled }
            }
        };

        Ok(())
    }

    pub fn locking_process(&self) -> Result<Option<i32>, PCIe40StreamError> {
        trace!(
            "Checking locking process for stream {} on device {}",
            self.stream_type, self.device_id
        );

        trace!("Calling p40_stream_get_locker({})", self.stream_fd);
        let c_result = unsafe { p40_stream_get_locker(self.stream_fd) };
        trace!("p40_stream_get_locker returned {}", c_result);

        if c_result == 0 {
            trace!(
                "Stream {} on device {} is not locked",
                self.stream_type, self.device_id
            );
            Ok(None)
        } else if c_result > 0 {
            debug!(
                "Stream {} on device {} is locked by process {}",
                self.stream_type, self.device_id, c_result
            );
            Ok(Some(c_result))
        } else {
            error!(
                "Unable to read locking process for stream {} on device {}",
                self.stream_type, self.device_id
            );
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.device_id,
                stream_type: self.stream_type,
                info: "Unable to read locking process".to_string(),
            })
        }
    }

    pub fn lock(&mut self) -> Result<PCIe40StreamGuard, PCIe40StreamError> {
        debug!(
            "Attempting to lock stream {} on device {}",
            self.stream_type, self.device_id
        );
        self.enable()?;

        trace!("Calling p40_stream_lock({})", self.stream_fd);
        let c_result = unsafe { p40_stream_lock(self.stream_fd) };
        trace!("p40_stream_lock returned {}", c_result);

        if c_result == 0 {
            info!(
                "Successfully locked stream {} on device {}",
                self.stream_type, self.device_id
            );
            Ok(PCIe40StreamGuard::new(self)?)
        } else if c_result > 0 {
            error!(
                "Failed to lock stream {} on device {} (already locked by process {})",
                self.stream_type, self.device_id, c_result
            );
            Err(PCIe40StreamError::FailedToLockStream {
                device_id: self.device_id,
                stream_type: self.stream_type,
            })
        } else {
            error!(
                "Error writing lock for stream {} on device {}",
                self.stream_type, self.device_id
            );
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.device_id,
                stream_type: self.stream_type,
                info: "Could not write lock".to_string(),
            })
        }
    }

    fn enable(&mut self) -> Result<(), PCIe40StreamError> {
        debug!(
            "Enabling stream {} on device {}",
            self.stream_type, self.device_id
        );
        if self.enabled()? {
            debug!(
                "Stream {} on device {} already enabled",
                self.stream_type, self.device_id
            );
            Ok(())
        } else {
            trace!("Calling p40_stream_enable({})", self.stream_fd);
            let c_result = unsafe { p40_stream_enable(self.stream_fd) };
            trace!("p40_stream_enable returned {}", c_result);

            if c_result == 0 {
                info!(
                    "Successfully enabled stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Ok(())
            } else {
                error!(
                    "Failed to enable stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Err(PCIe40StreamError::FailedToEnableStream {
                    device_id: self.device_id,
                    stream_type: self.stream_type,
                })
            }
        }
    }

    fn disable(&mut self) -> Result<(), PCIe40StreamError> {
        debug!(
            "Disabling stream {} on device {}",
            self.stream_type, self.device_id
        );
        if !self.enabled()? {
            debug!(
                "Stream {} on device {} already disabled",
                self.stream_type, self.device_id
            );
            Ok(())
        } else {
            trace!("Calling p40_stream_disable({})", self.stream_fd);
            let c_result = unsafe { p40_stream_disable(self.stream_fd) };
            trace!("p40_stream_disable returned {}", c_result);

            if c_result != 0 {
                error!(
                    "Failed to disable stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Err(PCIe40StreamError::FailedToDisableStream {
                    device_id: self.device_id,
                    stream_type: self.stream_type,
                })
            } else {
                info!(
                    "Successfully disabled stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Ok(())
            }
        }
    }

    fn run_raii_enable_state_action(&mut self) {
        match self.enable_state_action_on_close {
            PCIe40StreamHandleEnableStateActionOnClose::DisableOnClose => {
                debug!(
                    "Disabling stream {} on device {}",
                    self.stream_type, self.device_id
                );
                if let Err(e) = self.disable() {
                    error!("Failed to disable stream during Drop: {}", e);
                }
            }
            PCIe40StreamHandleEnableStateActionOnClose::EnableOnClose => {
                debug!(
                    "Keeping stream {} on device {} enabled",
                    self.stream_type, self.device_id
                );
                /* Do nothing... already enabled */
            }
            PCIe40StreamHandleEnableStateActionOnClose::PreserveEnableState { enabled } => {
                if !enabled {
                    debug!(
                        "Preserving disabled state for stream {} on device {}",
                        self.stream_type, self.device_id
                    );
                    if let Err(e) = self.disable() {
                        error!("Failed to disable stream during Drop: {}", e);
                    }
                } else {
                    debug!(
                        "Preserving enabled state for stream {} on device {}",
                        self.stream_type, self.device_id
                    );
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
        trace!(
            "Drop called on PCIe40StreamGuard for device {} stream {}",
            self.stream_handle.device_id, self.stream_handle.stream_type
        );
        if let Err(e) = self.ref_unlock() {
            error!("Failed to unlock stream during Drop: {}", e);
        }
    }
}

impl<'a> PCIe40StreamGuard<'a> {
    fn new(stream_handle: &'a mut PCIe40Stream) -> Result<Self, PCIe40StreamError> {
        debug!(
            "Creating new PCIe40StreamGuard for stream {} on device {}",
            stream_handle.stream_type, stream_handle.device_id
        );
        let mut locked_stream = PCIe40StreamGuard { stream_handle };

        match locked_stream.stream_handle.stream_format {
            PCIe40DAQStreamFormat::RawFormat => {
                debug!("Setting meta enabled state to false");
                locked_stream.set_meta_enabled_state(false)?
            }
            PCIe40DAQStreamFormat::MetaFormat => {
                debug!("Setting meta enabled state to true");
                locked_stream.set_meta_enabled_state(true)?
            }
        };

        Ok(locked_stream)
    }

    fn set_meta_enabled_state(&mut self, enabled: bool) -> Result<(), PCIe40StreamError> {
        debug!(
            "{} meta sub-stream for stream {} on device {}",
            if enabled { "Enabling" } else { "Disabling" },
            self.stream_handle.stream_type,
            self.stream_handle.device_id
        );

        trace!("Calling p40_stream_id_to_meta_mask({}, {:?})",
            self.stream_handle.device_id,
            self.stream_handle.stream_type
        );
        let meta_mask = unsafe {
            p40_stream_id_to_meta_mask(
                self.stream_handle.device_id,
                self.stream_handle.stream_type.into(),
            )
        };
        trace!("p40_stream_id_to_meta_mask returned {:#x}", meta_mask);
        trace!("Meta mask: {:#x}", meta_mask);

        let c_result = match enabled {
            true => {
                trace!("Calling p40_stream_enable_mask({}, {:#x})",
                    self.stream_handle.meta_stream_fd,
                    meta_mask
                );
                let result = unsafe { p40_stream_enable_mask(self.stream_handle.meta_stream_fd, meta_mask) };
                trace!("p40_stream_enable_mask returned {}", result);
                result
            },
            false => {
                trace!("Calling p40_stream_disable_mask({}, {:#x})",
                    self.stream_handle.meta_stream_fd,
                    meta_mask
                );
                let result = unsafe { p40_stream_disable_mask(self.stream_handle.meta_stream_fd, meta_mask) };
                trace!("p40_stream_disable_mask returned {}", result);
                result
            },
        };

        if c_result != 0 {
            error!(
                "Failed to {} meta bits for stream {} on device {}",
                if enabled { "enable" } else { "disable" },
                self.stream_handle.stream_type,
                self.stream_handle.device_id
            );
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
                info: "Unable to write meta bits".to_string(),
            })
        } else {
            debug!(
                "Successfully {} meta bits for stream {} on device {}",
                if enabled { "enabled" } else { "disabled" },
                self.stream_handle.stream_type,
                self.stream_handle.device_id
            );
            Ok(())
        }
    }

    fn ref_unlock(&mut self) -> Result<(), PCIe40StreamError> {
        debug!(
            "Unlocking stream {} on device {}",
            self.stream_handle.stream_type, self.stream_handle.device_id
        );

        trace!("Calling p40_stream_unlock({})", self.stream_handle.stream_fd);
        let c_result = unsafe { p40_stream_unlock(self.stream_handle.stream_fd) };
        trace!("p40_stream_unlock returned {}", c_result);

        if c_result == 0 {
            info!(
                "Successfully unlocked stream {} on device {}",
                self.stream_handle.stream_type, self.stream_handle.device_id
            );
            Ok(())
        } else if c_result > 0 {
            error!(
                "Failed to unlock stream {} on device {} (locked by process {})",
                self.stream_handle.stream_type, self.stream_handle.device_id, c_result
            );
            Err(PCIe40StreamError::FailedToUnlockStream {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            })
        } else {
            error!(
                "Error writing unlock for stream {} on device {}",
                self.stream_handle.stream_type, self.stream_handle.device_id
            );
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
                info: "Unable to write unlock".to_string(),
            })
        }
    }
}

impl<'a> PCIe40StreamGuard<'a> {
    pub fn map_buffer<'buf>(
        &'buf mut self,
    ) -> Result<PCIe40MappedBuffer<'a, 'buf>, PCIe40StreamError> {
        debug!(
            "Mapping buffer for stream {} on device {}",
            self.stream_handle.stream_type, self.stream_handle.device_id
        );

        trace!("Calling p40_stream_map({})", self.stream_handle.stream_fd);
        let buff_ptr = unsafe { p40_stream_map(self.stream_handle.stream_fd) };
        trace!("p40_stream_map returned {:p}", buff_ptr);

        if buff_ptr.is_null() {
            error!(
                "Failed to map buffer for stream {} on device {}: null pointer",
                self.stream_handle.stream_type, self.stream_handle.device_id
            );
            return Err(PCIe40StreamError::FailedToMapBuffer {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            });
        }

        trace!("Calling p40_stream_get_host_buf_bytes({})", self.stream_handle.stream_fd);
        let buff_size = unsafe { p40_stream_get_host_buf_bytes(self.stream_handle.stream_fd) };
        trace!("p40_stream_get_host_buf_bytes returned {}", buff_size);

        if buff_size <= 0 {
            error!(
                "Failed to map buffer for stream {} on device {}: invalid buffer size {}",
                self.stream_handle.stream_type, self.stream_handle.device_id, buff_size
            );
            return Err(PCIe40StreamError::FailedToMapBuffer {
                device_id: self.stream_handle.device_id,
                stream_type: self.stream_handle.stream_type,
            });
        }

        debug!(
            "Successfully mapped buffer of size {} bytes for stream {} on device {}",
            buff_size, self.stream_handle.stream_type, self.stream_handle.device_id
        );

        PCIe40MappedBuffer::new(self, unsafe {
            slice::from_raw_parts(buff_ptr as *const u8, buff_size as usize)
        })
    }
}

pub struct PCIe40MappedBuffer<'guard, 'buf> {
    pub stream_guard: &'buf mut PCIe40StreamGuard<'guard>,
    pub buffer: &'buf [u8],
}

impl<'guard, 'buf> Drop for PCIe40MappedBuffer<'guard, 'buf> {
    fn drop(&mut self) {
        trace!(
            "Drop called on PCIe40MappedBuffer for device {} stream {}",
            self.stream_guard.stream_handle.device_id, self.stream_guard.stream_handle.stream_type
        );
        self.unmap_buffer();
    }
}

impl<'guard, 'buf> PCIe40MappedBuffer<'guard, 'buf> {
    fn new(
        stream_guard: &'buf mut PCIe40StreamGuard<'guard>,
        buffer: &'buf [u8],
    ) -> Result<Self, PCIe40StreamError> {
        debug!(
            "Creating new PCIe40MappedBuffer with size {} for stream {} on device {}",
            buffer.len(),
            stream_guard.stream_handle.stream_type,
            stream_guard.stream_handle.device_id
        );

        Ok(Self {
            stream_guard,
            buffer,
        })
    }

    fn unmap_buffer(&mut self) {
        debug!(
            "Unmapping buffer for stream {} on device {}",
            self.stream_guard.stream_handle.stream_type, self.stream_guard.stream_handle.device_id
        );

        trace!(
            "Calling p40_stream_unmap({}, {:p})",
            self.stream_guard.stream_handle.stream_fd,
            self.buffer.as_ptr() as *mut std::os::raw::c_void
        );
        unsafe {
            p40_stream_unmap(
                self.stream_guard.stream_handle.stream_fd,
                self.buffer.as_ptr() as *mut std::os::raw::c_void,
            )
        }
        debug!(
            "Successfully unmapped buffer for stream {} on device {}",
            self.stream_guard.stream_handle.stream_type, self.stream_guard.stream_handle.device_id
        );
    }
}

impl<'guard, 'buf> PCIe40MappedBuffer<'guard, 'buf> {
    pub unsafe fn data(&self) -> &[u8] {
        trace!(
            "Accessing buffer data of size {} for stream {} on device {}",
            self.buffer.len(),
            self.stream_guard.stream_handle.stream_type,
            self.stream_guard.stream_handle.device_id
        );
        self.buffer
    }

    pub fn get_read_offset(&self) -> Result<usize, PCIe40StreamError> {
        trace!(
            "Getting buffer read offset for stream {:?} on device {}",
            self.stream_guard.stream_handle.stream_type, self.stream_guard.stream_handle.device_id
        );

        trace!("Calling p40_stream_get_host_buf_read_off({})", self.stream_guard.stream_handle.stream_fd);
        let offset = unsafe {
            p40_stream_get_host_buf_read_off(self.stream_guard.stream_handle.stream_fd)
        };
        trace!("p40_stream_get_host_buf_read_off returned {}", offset);

        if offset < 0 {
            error!(
                "Failed to get buffer read offset for stream {} on device {}",
                self.stream_guard.stream_handle.stream_type,
                self.stream_guard.stream_handle.device_id,
            );
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.stream_guard.stream_handle.device_id,
                stream_type: self.stream_guard.stream_handle.stream_type,
                info: "Unable to get buffer read offset".to_string(),
            })
        } else {
            debug!(
                "Buffer read offset for stream {} on device {}: {}",
                self.stream_guard.stream_handle.stream_type,
                self.stream_guard.stream_handle.device_id,
                offset
            );
            Ok(offset as usize)
        }
    }

    pub fn get_write_offset(&self) -> Result<usize, PCIe40StreamError> {
        trace!(
            "Getting buffer write offset for stream {} on device {}",
            self.stream_guard.stream_handle.stream_type, self.stream_guard.stream_handle.device_id
        );

        trace!("Calling p40_stream_get_host_buf_write_off({})", self.stream_guard.stream_handle.stream_fd);
        let offset = unsafe {
            p40_stream_get_host_buf_write_off(self.stream_guard.stream_handle.stream_fd)
        };
        trace!("p40_stream_get_host_buf_write_off returned {}", offset);

        if offset < 0 {
            error!(
                "Failed to get buffer write offset for stream {} on device {}",
                self.stream_guard.stream_handle.stream_type,
                self.stream_guard.stream_handle.device_id,
            );
            Err(PCIe40StreamError::StreamReadError {
                device_id: self.stream_guard.stream_handle.device_id,
                stream_type: self.stream_guard.stream_handle.stream_type,
                info: "Unable to get buffer write offset".to_string(),
            })
        } else {
            debug!(
                "Buffer write offset for stream {:?} on device {}: {}",
                self.stream_guard.stream_handle.stream_type,
                self.stream_guard.stream_handle.device_id,
                offset
            );
            Ok(offset as usize)
        }
    }

    pub fn move_read_offset(&mut self, offset: usize) -> Result<usize, PCIe40StreamError> {
        trace!(
            "Moving buffer read offset for stream {} on device {}",
            self.stream_guard.stream_handle.stream_type, self.stream_guard.stream_handle.device_id
        );

        trace!(
            "Calling p40_stream_free_host_buf_bytes({}, {})",
            self.stream_guard.stream_handle.stream_fd,
            offset
        );
        let offset = unsafe {
            p40_stream_free_host_buf_bytes(self.stream_guard.stream_handle.stream_fd, offset)
        };
        trace!("p40_stream_free_host_buf_bytes returned {}", offset);

        if offset < 0 {
            error!(
                "Failed to move buffer read offset for stream {} on device {}",
                self.stream_guard.stream_handle.stream_type,
                self.stream_guard.stream_handle.device_id,
            );
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.stream_guard.stream_handle.device_id,
                stream_type: self.stream_guard.stream_handle.stream_type,
                info: "Unable to move buffer read offset".to_string(),
            })
        } else {
            debug!(
                "Buffer read offset for stream {} on device {} moved {} bytes",
                self.stream_guard.stream_handle.stream_type,
                self.stream_guard.stream_handle.device_id,
                offset
            );
            Ok(offset as usize)
        }
    }
}

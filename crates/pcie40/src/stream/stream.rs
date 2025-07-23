use crate::bindings::*;
use crate::id::PCIe40IdManager;
use crate::stream::locked_stream::PCIe40LockedStream;
use std::fmt::{Display, Formatter};
use std::ptr;
use thiserror::Error;
use tracing::{debug, instrument, trace, warn};

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
    #[instrument]
    pub fn stream_exists(
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    ) -> Result<bool, PCIe40StreamManagerError> {
        debug!(
            "Checking if stream {} exists for device {}",
            stream_type, device_id
        );

        trace!(
            "Calling p40_stream_exists({}, {:?})",
            device_id, stream_type
        );
        let c_result = unsafe { p40_stream_exists(device_id, stream_type.into()) };
        trace!("p40_stream_exists returned {}", c_result);

        match c_result.cmp(&0) {
            std::cmp::Ordering::Less => {
                warn!(
                    "Driver read error while checking if stream {} exists for device {}",
                    stream_type, device_id
                );
                Err(PCIe40StreamManagerError::DriverReadError)
            }
            std::cmp::Ordering::Equal => {
                debug!("Stream {} exists for device {}", stream_type, device_id);
                Ok(true)
            }
            std::cmp::Ordering::Greater => {
                debug!(
                    "Stream {} does not exist for device {}",
                    stream_type, device_id
                );
                Ok(false)
            }
        }
    }

    #[instrument]
    pub fn open_by_device_name(
        device_name: &str,
        stream_type: PCIe40DAQStreamType,
        stream_format: PCIe40DAQStreamFormat,
    ) -> Result<PCIe40Stream, PCIe40StreamManagerError> {
        debug!(
            "Opening stream {} with format {} by device name '{}'",
            stream_type, stream_format, device_name
        );

        let device_id = PCIe40IdManager::find_id_by_name(device_name).map_err(|_| {
            warn!("Device with name '{}' not found", device_name);
            PCIe40StreamManagerError::DeviceNotFoundByName {
                device_name: device_name.into(),
            }
        })?;

        Self::open_by_device_id(device_id, stream_type, stream_format)
    }

    #[instrument]
    pub fn open_by_device_id(
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
        stream_format: PCIe40DAQStreamFormat,
    ) -> Result<PCIe40Stream, PCIe40StreamManagerError> {
        debug!(
            "Opening stream {} with format {} by device ID {}",
            stream_type, stream_format, device_id
        );

        if !Self::stream_exists(device_id, stream_type)? {
            warn!("Device with id {} not found", device_id);
            return Err(PCIe40StreamManagerError::DeviceNotFoundById { device_id });
        }

        trace!("Calling p40_stream_open({}, {:?})", device_id, stream_type);
        let stream_fd = unsafe { p40_stream_open(device_id, stream_type.into()) };
        trace!("p40_stream_open returned {}", stream_fd);

        if stream_fd < 0 {
            warn!(
                "Failed to open stream {} for device {}",
                stream_type, device_id
            );
            return Err(PCIe40StreamManagerError::StreamOpenFail {
                device_id,
                stream_type,
            });
        }
        debug!("Opened {} stream with fd {}", stream_type, stream_fd);

        trace!(
            "Calling p40_stream_open({}, {:?}) for meta stream",
            device_id, stream_type
        );
        let meta_stream_fd =
            unsafe { p40_stream_open(device_id, P40_DAQ_STREAM_P40_DAQ_STREAM_META) };
        trace!(
            "p40_stream_open for meta stream returned {}",
            meta_stream_fd
        );

        if meta_stream_fd < 0 {
            warn!(
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

        debug!(
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

        trace!(
            "Calling p40_stream_close({}, MetaStream)",
            stream_endpoint.stream_fd
        );
        unsafe {
            p40_stream_close(stream_endpoint.stream_fd, ptr::null_mut());
        }

        trace!(
            "Calling p40_stream_close({}, {:?})",
            stream_endpoint.stream_fd, stream_endpoint.stream_type
        );
        unsafe {
            p40_stream_close(stream_endpoint.stream_fd, ptr::null_mut());
        }

        debug!("Closed stream fd {}", stream_endpoint.stream_fd);
    }
}

pub struct PCIe40Stream {
    pub(super) device_id: i32,
    pub(super) stream_fd: i32,
    pub(super) meta_stream_fd: i32,
    pub(super) stream_type: PCIe40DAQStreamType,
    pub(super) stream_format: PCIe40DAQStreamFormat,
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
    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type
    ))]
    fn drop(&mut self) {
        debug!(
            "Drop called on PCIe40Stream for device {} stream {}",
            self.device_id, self.stream_type
        );
        self.run_raii_enable_state_action();
        PCIe40StreamManager::close(self);
    }
}

impl PCIe40Stream {
    #[instrument]
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

    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    pub fn stream_type(&self) -> PCIe40DAQStreamType {
        self.stream_type
    }

    pub fn stream_format(&self) -> PCIe40DAQStreamFormat {
        self.stream_format
    }

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type
    ))]
    pub fn enabled(&self) -> Result<bool, PCIe40StreamError> {
        debug!(
            "Checking if stream {} on device {} is enabled",
            self.stream_type, self.device_id
        );

        trace!("Calling p40_stream_enabled({})", self.stream_fd);
        let c_result = unsafe { p40_stream_enabled(self.stream_fd) };
        trace!("p40_stream_enabled returned {}", c_result);

        if c_result < 0 {
            warn!(
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

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type,
        preserve_mode = ?preserve_mode
    ))]
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

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type,
    ))]
    pub fn locking_process(&self) -> Result<Option<i32>, PCIe40StreamError> {
        debug!(
            "Checking locking process for stream {} on device {}",
            self.stream_type, self.device_id
        );

        trace!("Calling p40_stream_get_locker({})", self.stream_fd);
        let c_result = unsafe { p40_stream_get_locker(self.stream_fd) };
        trace!("p40_stream_get_locker returned {}", c_result);

        match c_result.cmp(&0) {
            std::cmp::Ordering::Equal => {
                trace!(
                    "Stream {} on device {} is not locked",
                    self.stream_type, self.device_id
                );
                Ok(None)
            }
            std::cmp::Ordering::Greater => {
                debug!(
                    "Stream {} on device {} is locked by process {}",
                    self.stream_type, self.device_id, c_result
                );
                Ok(Some(c_result))
            }
            std::cmp::Ordering::Less => {
                warn!(
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
    }

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type,
    ))]
    pub fn lock(mut self) -> Result<PCIe40LockedStream, PCIe40StreamError> {
        debug!(
            "Attempting to lock stream {} on device {}",
            self.stream_type, self.device_id
        );
        self.enable()?;

        match self.stream_format {
            PCIe40DAQStreamFormat::RawFormat => {
                debug!("Setting meta enabled state to false");
                self.set_meta_enabled_state(false)?
            }
            PCIe40DAQStreamFormat::MetaFormat => {
                debug!("Setting meta enabled state to true");
                self.set_meta_enabled_state(true)?
            }
        };

        trace!("Calling p40_stream_lock({})", self.stream_fd);
        let c_result = unsafe { p40_stream_lock(self.stream_fd) };
        trace!("p40_stream_lock returned {}", c_result);

        match c_result.cmp(&0) {
            std::cmp::Ordering::Equal => {
                debug!(
                    "Successfully locked stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Ok(PCIe40LockedStream::new(self))
            }
            std::cmp::Ordering::Greater => {
                warn!(
                    "Failed to lock stream {} on device {} (already locked by process {})",
                    self.stream_type, self.device_id, c_result
                );
                Err(PCIe40StreamError::StreamWriteError {
                    device_id: self.device_id,
                    stream_type: self.stream_type,
                    info: format!("Stream already locked by process {}", c_result),
                })
            }
            std::cmp::Ordering::Less => {
                warn!(
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
    }

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type,
        enabled = enabled,
    ))]
    fn set_meta_enabled_state(&mut self, enabled: bool) -> Result<(), PCIe40StreamError> {
        debug!(
            "{} meta sub-stream for stream {} on device {}",
            if enabled { "Enabling" } else { "Disabling" },
            self.stream_type,
            self.device_id
        );

        trace!(
            "Calling p40_stream_id_to_meta_mask({}, {:?})",
            self.device_id, self.stream_type
        );
        let meta_mask =
            unsafe { p40_stream_id_to_meta_mask(self.device_id, self.stream_type.into()) };
        trace!("p40_stream_id_to_meta_mask returned {:#x}", meta_mask);
        trace!("Meta mask: {:#x}", meta_mask);

        let c_result = match enabled {
            true => {
                trace!(
                    "Calling p40_stream_enable_mask({}, {:#x})",
                    self.meta_stream_fd, meta_mask
                );
                let result = unsafe { p40_stream_enable_mask(self.meta_stream_fd, meta_mask) };
                trace!("p40_stream_enable_mask returned {}", result);
                result
            }
            false => {
                trace!(
                    "Calling p40_stream_disable_mask({}, {:#x})",
                    self.meta_stream_fd, meta_mask
                );
                let result = unsafe { p40_stream_disable_mask(self.meta_stream_fd, meta_mask) };
                trace!("p40_stream_disable_mask returned {}", result);
                result
            }
        };

        if c_result != 0 {
            warn!(
                "Failed to {} meta bits for stream {} on device {}",
                if enabled { "enable" } else { "disable" },
                self.stream_type,
                self.device_id
            );
            Err(PCIe40StreamError::StreamWriteError {
                device_id: self.device_id,
                stream_type: self.stream_type,
                info: "Unable to write meta bits".to_string(),
            })
        } else {
            debug!(
                "Successfully {} meta bits for stream {} on device {}",
                if enabled { "enabled" } else { "disabled" },
                self.stream_type,
                self.device_id
            );
            Ok(())
        }
    }

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type,
    ))]
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
                debug!(
                    "Successfully enabled stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Ok(())
            } else {
                warn!(
                    "Failed to enable stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Err(PCIe40StreamError::StreamWriteError {
                    device_id: self.device_id,
                    stream_type: self.stream_type,
                    info: "Failed to enable stream".to_string(),
                })
            }
        }
    }

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type,
    ))]
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
                warn!(
                    "Failed to disable stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Err(PCIe40StreamError::StreamWriteError {
                    device_id: self.device_id,
                    stream_type: self.stream_type,
                    info: "Failed to disable stream".to_string(),
                })
            } else {
                debug!(
                    "Successfully disabled stream {} on device {}",
                    self.stream_type, self.device_id
                );
                Ok(())
            }
        }
    }

    #[instrument(skip_all, fields(
        device_id = self.device_id,
        stream_type = ?self.stream_type,
    ))]
    fn run_raii_enable_state_action(&mut self) {
        match self.enable_state_action_on_close {
            PCIe40StreamHandleEnableStateActionOnClose::DisableOnClose => {
                debug!(
                    "Disabling stream {} on device {}",
                    self.stream_type, self.device_id
                );
                if let Err(e) = self.disable() {
                    warn!("Failed to disable stream during Drop: {}", e);
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
                        warn!("Failed to disable stream during Drop: {}", e);
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

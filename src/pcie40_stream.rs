use crate::bindings::*;
use crate::pcie40_id::{PCIe40Id, PCIe40IdError};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PCIe40DAQStreamType {
    NullStream,
    MainStream,
    MetaStream,
    Odin0Stream,
    Odin1Stream,
    Odin2Stream,
    Odin3Stream,
    Odin4Stream,
}

impl From<P40_DAQ_STREAM> for PCIe40DAQStreamType {
    fn from(value: P40_DAQ_STREAM) -> Self {
        match value {
            P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN => PCIe40DAQStreamType::MainStream,
            P40_DAQ_STREAM_P40_DAQ_STREAM_META => PCIe40DAQStreamType::MetaStream,
            P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN0 => PCIe40DAQStreamType::Odin0Stream,
            P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN1 => PCIe40DAQStreamType::Odin1Stream,
            P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN2 => PCIe40DAQStreamType::Odin2Stream,
            P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN3 => PCIe40DAQStreamType::Odin3Stream,
            P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN4 => PCIe40DAQStreamType::Odin4Stream,
            _ => PCIe40DAQStreamType::NullStream,
        }
    }
}

impl From<PCIe40DAQStreamType> for P40_DAQ_STREAM {
    fn from(value: PCIe40DAQStreamType) -> Self {
        match value {
            PCIe40DAQStreamType::NullStream => P40_DAQ_STREAM_P40_DAQ_STREAM_NULL,
            PCIe40DAQStreamType::MainStream => P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN,
            PCIe40DAQStreamType::MetaStream => P40_DAQ_STREAM_P40_DAQ_STREAM_META,
            PCIe40DAQStreamType::Odin0Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN0,
            PCIe40DAQStreamType::Odin1Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN1,
            PCIe40DAQStreamType::Odin2Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN2,
            PCIe40DAQStreamType::Odin3Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN3,
            PCIe40DAQStreamType::Odin4Stream => P40_DAQ_STREAM_P40_DAQ_STREAM_ODIN4,
        }
    }
}

pub struct PCIe40Stream {}

#[derive(Debug, Error)]
pub enum PCIe40CStreamError {
    #[error("Invalid device name: {device_name}")]
    InvalidDeviceName { device_name: String },

    #[error("Error reading from PCIe40 drivers")]
    DriverReadError,

    #[error("Device with name \"{device_name}\" not found")]
    DeviceNotFoundByName { device_name: String },

    #[error("Device with id {device_id} not found")]
    DeviceNotFoundById { device_id: i32 },

    #[error("Failed to open PCIE40 device with id {device_id}")]
    DeviceOpenFail { device_id: i32 },
}

impl PCIe40Stream {
    pub fn stream_exists(
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    ) -> Result<bool, PCIe40CStreamError> {
        let c_result = unsafe { p40_stream_exists(device_id, stream_type.into()) };

        if c_result < 0 {
            Err(PCIe40CStreamError::DriverReadError)
        } else if c_result == 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn open_by_device_name(
        device_name: &str,
        stream_type: PCIe40DAQStreamType,
    ) -> Result<PCIe40StreamEndpoint, PCIe40CStreamError> {
        let device_id = PCIe40Id::find_id_by_name(device_name).or(Err(
            PCIe40CStreamError::DeviceNotFoundByName {
                device_name: device_name.into(),
            },
        ))?;

        Self::open_by_device_id(device_id, stream_type)
    }

    pub fn open_by_device_id(
        device_id: i32,
        stream_type: PCIe40DAQStreamType,
    ) -> Result<PCIe40StreamEndpoint, PCIe40CStreamError> {
        let stream_fd = unsafe { p40_stream_open(device_id, stream_type.into()) };
        if stream_fd < 0 {
            Err(PCIe40CStreamError::DeviceNotFoundById { device_id })?;
        }

        Ok(PCIe40StreamEndpoint { stream_fd })
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(stream_endpoint: &mut PCIe40StreamEndpoint) {
        unsafe {
            p40_ctrl_close(stream_endpoint.stream_fd);
        }
    }
}

pub struct PCIe40StreamEndpoint {
    stream_fd: i32,
}

#[derive(Debug, Error)]
pub enum PCIe40StreamEndpointError {
    #[error("Error reading data from the PCIe40 device with id {device_id}")]
    DeviceReadError { device_id: i32 },

    #[error("Error write data to the PCIe40 device with id {device_id}")]
    DeviceWriteError { device_id: i32 },

    #[error("Invalid device name: {device_name}")]
    InvalidDeviceName { device_name: String },
}

impl Drop for PCIe40StreamEndpoint {
    fn drop(&mut self) {
        PCIe40Stream::close(self);
    }
}

impl PCIe40StreamEndpoint {}

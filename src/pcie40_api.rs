use std::ffi::CString;
use std::io;
use thiserror::Error;
use crate::bindings::*;
use crate::pcie40_api::PCIe40DeviceIdError::DeviceReadError;
use crate::pcie40_api::PCIe40IdError::{DeviceNotFoundByName, DriverReadError, InvalidName};

/// Errors that can occur when working with PCIe40 devices
#[derive(Debug, Error)]
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

struct PCIe40Id {}

#[derive(Debug, Error)]
enum PCIe40IdError {
    #[error("Invalid device name: {0}")]
    InvalidName(String),

    #[error("Error reading from PCIe40 drivers")]
    DriverReadError,

    #[error("Device with name \"{0}\" not found")]
    DeviceNotFoundByName(String),

    #[error("Device with id {0} not found")]
    DeviceNotFoundById(i32),
}

impl PCIe40Id {
    pub fn id_exists(device_id: i32) -> Result<bool, PCIe40IdError> {
        let c_result = unsafe { p40_id_exists(device_id) };

        if c_result < 0 {
            Err(DriverReadError)
        } else if c_result == 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn find_id_by_name(device_name: &str) -> Result<i32, PCIe40IdError> {
        let c_str_device_name = CString::new(device_name).map_err(|_| InvalidName(device_name.to_string()))?;
        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) };

        if device_id < 0 {
            Err(DeviceNotFoundByName(device_name.to_string()))
        } else {
            Ok(device_id)
        }
    }

    pub fn find_all_ids_by_name(device_name: &str) -> Result<Vec<i32>, PCIe40IdError> {
        let c_str_device_name = CString::new(device_name).map_err(|_| PCIe40IdError::InvalidName(device_name.to_string()))?;
        let mask = unsafe { p40_id_find_all(c_str_device_name.as_ptr()) };

        Ok((0..32)
            .filter(|&device_id| (mask & (1 << device_id)) != 0)
            .map(|device_id| device_id as i32)
            .collect())
    }

    /*
    pub fn fpga_serial_number()

    pub fn open_by_name(device_name: &str) -> Result<Self, PCIe40Error> {
        let c_str_device_name = CString::new(device_name)
            .map_err(PCIe40Error::DeviceNotFound(device_name.to_string()))?;

        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) });
        if device_id < 0 {
            Err(PCIe40Error::DeviceNotFound(device_name.to_string()))?;
        };

        Self::open(device_name, device_id)
    }

    pub fn open_by_id(device_id: i32) -> Result<Self, PCIe40Error> {
        if unsafe { p40_id_exists(device_id) } != 0 {
            Err(PCIe40Error::DeviceNotFound(device_id.to_string()))?;
        }

        Self::open("Unkown", device_id)
    }

    fn open(device_name: &str, device_id: i32) -> Result<Self, PCIe40Error> {}

     */
}

struct PCIe40DeviceId {
    device_name: String,
    device_id: i32,
    id_fd: i32,
}

#[derive(Debug, Error)]
enum PCIe40DeviceIdError {
    #[error("Error reading data from the PCIe40 device with name \"{device_name}\" and id {device_id}")]
    DeviceReadError { device_name: String, device_id: i32 },
}

impl PCIe40DeviceId{
    pub fn fpga_serial_number(&self) -> Result<i64, PCIe40DeviceIdError> {
        let c_result = unsafe { p40_id_get_fpga(self.id_fd) };
        if c_result == -1 {
            return Err(DeviceReadError { device_name: self.device_name.clone(), device_id: self.device_id });
        }

        Ok(c_result)
    }

    // TODO: front_panel_leds_status -> p40_id_get_leds
    // TODO: pcie_link_id -> p40_id_get_link


}

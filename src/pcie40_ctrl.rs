use crate::bindings::*;
use crate::pcie40_id::{PCIe40Id, PCIe40IdError};
use thiserror::Error;

pub struct PCIe40Ctrl {}

#[derive(Debug, Error)]
pub enum PCIe40CtrlError {
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

impl PCIe40Ctrl {
    pub fn controller_exists(device_id: i32) -> Result<bool, PCIe40CtrlError> {
        let c_result = unsafe { p40_id_exists(device_id) };

        if c_result < 0 {
            Err(PCIe40CtrlError::DriverReadError)
        } else if c_result == 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn open_by_device_name(device_name: &str) -> Result<PCIe40CtrlEndpoint, PCIe40CtrlError> {
        let device_id = PCIe40Id::find_id_by_name(device_name).or(Err(
            PCIe40CtrlError::DeviceNotFoundByName {
                device_name: device_name.into(),
            },
        ))?;

        Self::open_by_device_id(device_id)
    }

    pub fn open_by_device_id(device_id: i32) -> Result<PCIe40CtrlEndpoint, PCIe40CtrlError> {
        let ctrl_fd = unsafe { p40_ctrl_open(device_id) };
        if ctrl_fd < 0 {
            Err(PCIe40CtrlError::DeviceNotFoundById { device_id })?;
        }

        Ok(PCIe40CtrlEndpoint { ctrl_fd, device_id })
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(ctrl_endpoint: &mut PCIe40CtrlEndpoint) {
        unsafe { p40_ctrl_close(ctrl_endpoint.ctrl_fd); }
    }
}

pub struct PCIe40CtrlEndpoint {
    ctrl_fd: i32,
    device_id: i32,
}

#[derive(Debug, Error)]
pub enum PCIe40CtrlEndpointError {
    #[error("Error reading data from the PCIe40 device with id {device_id}")]
    DeviceReadError { device_id: i32 },

    #[error("Error write data to the PCIe40 device with id {device_id}")]
    DeviceWriteError { device_id: i32 },

    #[error("Invalid device name: {device_name}")]
    InvalidDeviceName { device_name: String },
}

impl Drop for PCIe40CtrlEndpoint {
    fn drop(&mut self) {
        PCIe40Ctrl::close(self);
    }
}

impl PCIe40CtrlEndpoint {}

use crate::bindings::*;
use crate::pcie40_id::PCIe40IdManager;
use thiserror::Error;

pub struct PCIe40ControllerManager {}

#[derive(Debug, Error)]
pub enum PCIe40ControllerManagerError {
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

impl PCIe40ControllerManager {
    pub fn controller_exists(device_id: i32) -> Result<bool, PCIe40ControllerManagerError> {
        let c_result = unsafe { p40_id_exists(device_id) };

        if c_result < 0 {
            Err(PCIe40ControllerManagerError::DriverReadError)
        } else if c_result == 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn open_by_device_name(device_name: &str) -> Result<PCIe40Controller, PCIe40ControllerManagerError> {
        let device_id = PCIe40IdManager::find_id_by_name(device_name).or(Err(
            PCIe40ControllerManagerError::DeviceNotFoundByName {
                device_name: device_name.into(),
            },
        ))?;

        Self::open_by_device_id(device_id)
    }

    pub fn open_by_device_id(device_id: i32) -> Result<PCIe40Controller, PCIe40ControllerManagerError> {
        let ctrl_fd = unsafe { p40_ctrl_open(device_id) };
        if ctrl_fd < 0 {
            Err(PCIe40ControllerManagerError::DeviceNotFoundById { device_id })?;
        }

        Ok(PCIe40Controller { ctrl_fd, device_id })
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(ctrl_endpoint: &mut PCIe40Controller) {
        unsafe { p40_ctrl_close(ctrl_endpoint.ctrl_fd); }
    }
}

pub struct PCIe40Controller {
    ctrl_fd: i32,
    device_id: i32,
}

#[derive(Debug, Error)]
pub enum PCIe40ControllerError {
    #[error("Error reading data from the PCIe40 device with id {device_id}")]
    DeviceReadError { device_id: i32 },

    #[error("Error write data to the PCIe40 device with id {device_id}")]
    DeviceWriteError { device_id: i32 },

    #[error("Invalid device name: {device_name}")]
    InvalidDeviceName { device_name: String },
}

impl Drop for PCIe40Controller {
    fn drop(&mut self) {
        PCIe40ControllerManager::close(self);
    }
}

impl PCIe40Controller {}

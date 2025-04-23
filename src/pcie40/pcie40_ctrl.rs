use crate::pcie40::bindings::*;
use crate::pcie40::pcie40_id::PCIe40IdManager;
use log::{debug, error, info, trace};
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
        debug!("Checking if controller with ID {} exists", device_id);

        trace!("Calling p40_ctrl_exists({})", device_id);
        let c_result = unsafe { p40_ctrl_exists(device_id) };
        trace!("p40_ctrl_exists returned {}", c_result);

        match c_result.cmp(&0) {
            std::cmp::Ordering::Less => {
                error!(
                    "Driver read error while checking if controller with ID {} exists",
                    device_id
                );
                Err(PCIe40ControllerManagerError::DriverReadError)
            }
            std::cmp::Ordering::Equal => {
                debug!("Controller with ID {} exists", device_id);
                Ok(true)
            }
            std::cmp::Ordering::Greater => {
                debug!("Controller with ID {} does not exist", device_id);
                Ok(false)
            }
        }
    }

    pub fn open_by_device_name(
        device_name: &str,
    ) -> Result<PCIe40Controller, PCIe40ControllerManagerError> {
        info!("Opening controller by device name '{}'", device_name);

        trace!(
            "Calling PCIe40IdManager::find_id_by_name(\"{}\")",
            device_name
        );
        let device_id = PCIe40IdManager::find_id_by_name(device_name).map_err(|_| {
            error!("Device with name '{}' not found", device_name);
            PCIe40ControllerManagerError::DeviceNotFoundByName {
                device_name: device_name.into(),
            }
        })?;
        trace!("PCIe40IdManager::find_id_by_name returned {}", device_id);

        Self::open_by_device_id(device_id)
    }

    pub fn open_by_device_id(
        device_id: i32,
    ) -> Result<PCIe40Controller, PCIe40ControllerManagerError> {
        info!("Opening controller for device with ID {}", device_id);

        trace!("Calling p40_ctrl_open({})", device_id);
        let ctrl_fd = unsafe { p40_ctrl_open(device_id) };
        trace!("p40_ctrl_open returned {}", ctrl_fd);

        if ctrl_fd < 0 {
            error!("Failed to open controller for device with ID {}", device_id);
            Err(PCIe40ControllerManagerError::DeviceNotFoundById { device_id })?;
        }

        debug!(
            "Successfully opened controller for device {} with fd {}",
            device_id, ctrl_fd
        );
        Ok(PCIe40Controller { ctrl_fd, device_id })
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(ctrl_endpoint: &mut PCIe40Controller) {
        debug!(
            "Closing controller for device {} with fd {}",
            ctrl_endpoint.device_id, ctrl_endpoint.ctrl_fd
        );

        trace!("Calling p40_ctrl_close({})", ctrl_endpoint.ctrl_fd);
        unsafe {
            p40_ctrl_close(ctrl_endpoint.ctrl_fd);
        }

        debug!("Closed controller for device {}", ctrl_endpoint.device_id);
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
        trace!(
            "Drop called on PCIe40Controller for device {}",
            self.device_id
        );
        PCIe40ControllerManager::close(self);
    }
}

impl PCIe40Controller {
    pub fn meta_alignment(&self) -> Result<usize, PCIe40ControllerError> {
        debug!("Getting meta alignment for device {}", self.device_id);

        trace!("Calling p40_ctrl_get_meta_alignment({})", self.ctrl_fd);
        let c_result = unsafe { p40_ctrl_get_meta_alignment(self.ctrl_fd) };
        trace!("p40_ctrl_get_meta_alignment returned {}", c_result);

        if c_result < 0 {
            error!("Failed to get meta alignment for device {}", self.device_id);
            return Err(PCIe40ControllerError::DeviceReadError {
                device_id: self.device_id,
            });
        }

        let alignment = c_result as usize;
        debug!(
            "Meta alignment for device {}: {}",
            self.device_id, alignment
        );

        Ok(alignment)
    }
}

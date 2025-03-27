use crate::bindings::*;
use std::ffi::CString;
use thiserror::Error;

pub struct PCIe40Id {}

#[derive(Debug, Error)]
pub enum PCIe40IdError {
    #[error("Invalid device name: {0}")]
    InvalidDeviceName(String),

    #[error("Error reading from PCIe40 drivers")]
    DriverReadError,

    #[error("Device with name \"{0}\" not found")]
    DeviceNotFoundByName(String),

    #[error("Device with id {0} not found")]
    DeviceNotFoundById(i32),

    #[error("Failed to open PCIE40 device \"{device_name}\" with id {device_id}")]
    DeviceOpenFail{device_name: String, device_id: i32},
}

impl PCIe40Id {
    pub fn id_exists(device_id: i32) -> Result<bool, PCIe40IdError> {
        let c_result = unsafe { p40_id_exists(device_id) };

        if c_result < 0 {
            Err(PCIe40IdError::DriverReadError)
        } else if c_result == 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn find_id_by_name(device_name: &str) -> Result<i32, PCIe40IdError> {
        let c_str_device_name =
            CString::new(device_name).map_err(|_| PCIe40IdError::InvalidDeviceName(device_name.to_string()))?;
        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) };

        if device_id < 0 {
            Err(PCIe40IdError::DeviceNotFoundByName(device_name.to_string()))
        } else {
            Ok(device_id)
        }
    }

    pub fn find_all_ids_by_name(device_name: &str) -> Result<Vec<i32>, PCIe40IdError> {
        let c_str_device_name = CString::new(device_name)
            .map_err(|_| PCIe40IdError::InvalidDeviceName(device_name.to_string()))?;
        let mask = unsafe { p40_id_find_all(c_str_device_name.as_ptr()) };

        Ok((0..32)
            .filter(|&device_id| (mask & (1 << device_id)) != 0)
            .map(|device_id| device_id as i32)
            .collect())
    }

    pub fn open_by_device_name(device_name: &str) -> Result<PCIe40DeviceId, PCIe40IdError> {
        let c_str_device_name = CString::new(device_name)
            .or(Err(PCIe40IdError::DeviceNotFoundByName(device_name.to_string())))?;

        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) };
        if device_id < 0 {
            Err(PCIe40IdError::DeviceNotFoundByName(device_name.to_string()))?;
        };

        Self::open(Some(device_name), device_id)
    }

    pub fn open_by_device_id(device_id: i32) -> Result<PCIe40DeviceId, PCIe40IdError> {
        if unsafe { p40_id_exists(device_id) } != 0 {
            Err(PCIe40IdError::DeviceNotFoundById(device_id))?;
        }

        Self::open(None, device_id)
    }

    fn open(device_name: Option<&str>, device_id: i32) -> Result<PCIe40DeviceId, PCIe40IdError> {
        let id_fd = unsafe { p40_id_open(device_id) };
        if id_fd < 0 {
            return Err(PCIe40IdError::DeviceOpenFail {
                device_name: device_name.unwrap_or("Unknown").to_string(),
                device_id,
            });
        }

        let mut device = PCIe40DeviceId {
            device_name: device_name.unwrap_or("Unkown").to_string(),
            device_id,
            id_fd
        };

        if device_name.is_none() {
            let _ = device.device_name();
        }

        Ok(device)
    }
}

pub struct PCIe40DeviceId {
    device_name: String,
    device_id: i32,
    id_fd: i32,
}

#[derive(Debug, Error)]
pub enum PCIe40DeviceIdError {
    #[error(
        "Error reading data from the PCIe40 device with name \"{device_name}\" and id {device_id}"
    )]
    DeviceReadError { device_name: String, device_id: i32 },

    #[error(
        "Error write data to the PCIe40 device with name \"{device_name}\" and id {device_id}"
    )]
    DeviceWriteError { device_name: String, device_id: i32 },

    #[error("Invalid device name: {0}")]
    InvalidDeviceName(String),
}

impl PCIe40DeviceId {
    pub fn fpga_serial_number(&self) -> Result<i64, PCIe40DeviceIdError> {
        let c_result = unsafe { p40_id_get_fpga(self.id_fd) };
        if c_result < 0 {
            return Err(PCIe40DeviceIdError::DeviceReadError {
                device_name: self.device_name.clone(),
                device_id: self.device_id,
            });
        }

        Ok(c_result)
    }

    pub fn device_name(&mut self) -> Result<String, PCIe40DeviceIdError> {
        let mut buffer = vec![0u8; 9];

        let c_result = unsafe {
            p40_id_get_name(
                self.id_fd,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
            )
        };

        if c_result < 0 {
            return Err(PCIe40DeviceIdError::DeviceReadError {
                device_name: self.device_name.clone(),
                device_id: self.device_id,
            });
        }

        self.device_name = self.c_buffer_to_string(&buffer)?;

        Ok(self.device_name.clone())
    }

    pub fn unique_device_name(&mut self) -> Result<String, PCIe40DeviceIdError> {
        let mut buffer = vec![0u8; 9];

        let c_result = unsafe {
            p40_id_get_name_unique(
                self.id_fd,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
            )
        };

        if c_result < 0 {
            return Err(PCIe40DeviceIdError::DeviceReadError {
                device_name: self.device_name.clone(),
                device_id: self.device_id,
            });
        }

        let unique_device_name = self.c_buffer_to_string(&buffer)?;

        Ok(unique_device_name)
    }

    pub fn set_name(&mut self, new_name: &str) -> Result<(), PCIe40DeviceIdError> {
        let c_str_name = CString::new(new_name)
            .map_err(|_| PCIe40DeviceIdError::InvalidDeviceName(new_name.to_string()))?;

        let c_result = unsafe {
            p40_id_set_name(self.id_fd, c_str_name.as_ptr())
        };

        if c_result < 0 {
            return Err(PCIe40DeviceIdError::DeviceWriteError {
                device_name: self.device_name.clone(),
                device_id: self.device_id,
            });
        }

        self.device_name = new_name.to_string();

        Ok(())
    }

    // TODO: front_panel_leds_status -> p40_id_get_leds
    // TODO: set_front_panel_leds -> p40_id_set_leds
    // TODO: pcie_link_id -> p40_id_get_link
    // TODO: register_map_version -> p40_id_get_regmap
    // TODO: read_test_register -> p40_id_get_rwtest
    // TODO: write_test_register -> p40_id_set_rwtest
    // TODO: unique_source_number -> p40_id_get_source
    // TODO: set_unique_source_number -> p40_id_set_source
    // TODO: pcie_version -> p40_id_get_version
}

impl PCIe40DeviceId {
    fn c_buffer_to_string(&self, buffer: &[u8]) -> Result<String, PCIe40DeviceIdError> {
        let null_pos = buffer
            .iter()
            .position(|&c| c == 0)
            .ok_or(PCIe40DeviceIdError::DeviceReadError {
                device_name: self.device_name.clone(),
                device_id: self.device_id,
            })?;

        Ok(String::from_utf8_lossy(&buffer[0..null_pos]).to_string())
    }
}

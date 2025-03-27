use crate::bindings::*;
use std::ffi::CString;
use thiserror::Error;

pub struct PCIe40Id {}

#[derive(Debug, Error)]
pub enum PCIe40IdError {
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

impl PCIe40Id {
    pub fn device_exists(device_id: i32) -> Result<bool, PCIe40IdError> {
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
            CString::new(device_name).map_err(|_| PCIe40IdError::InvalidDeviceName {
                device_name: device_name.to_string(),
            })?;
        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) };

        if device_id < 0 {
            Err(PCIe40IdError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            })
        } else {
            Ok(device_id)
        }
    }

    pub fn find_all_ids_by_name(device_name: &str) -> Result<Vec<i32>, PCIe40IdError> {
        let c_str_device_name =
            CString::new(device_name).map_err(|_| PCIe40IdError::InvalidDeviceName {
                device_name: device_name.to_string(),
            })?;
        let mask = unsafe { p40_id_find_all(c_str_device_name.as_ptr()) };

        Ok((0..32)
            .filter(|&device_id| (mask & (1 << device_id)) != 0)
            .map(|device_id| device_id as i32)
            .collect())
    }

    pub fn open_by_device_name(device_name: &str) -> Result<PCIe40IdStream, PCIe40IdError> {
        let c_str_device_name =
            CString::new(device_name).or(Err(PCIe40IdError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            }))?;

        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) };
        if device_id < 0 {
            Err(PCIe40IdError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            })?;
        };

        Self::open_by_device_id(device_id)
    }

    pub fn open_by_device_id(device_id: i32) -> Result<PCIe40IdStream, PCIe40IdError> {
        if unsafe { p40_id_exists(device_id) } != 0 {
            Err(PCIe40IdError::DeviceNotFoundById { device_id })?;
        }

        let id_fd = unsafe { p40_id_open(device_id) };
        if id_fd < 0 {
            Err(PCIe40IdError::DeviceOpenFail { device_id })?;
        }

        Ok(PCIe40IdStream { device_id, id_fd })
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(id_stream: &PCIe40IdStream) {
        unsafe { p40_id_close(id_stream.id_fd) };
    }
}

pub struct PCIe40IdStream {
    device_id: i32,
    id_fd: i32,
}

#[derive(Debug, Error)]
pub enum PCIe40IdStreamError {
    #[error("Error reading data from the PCIe40 device with id {device_id}")]
    DeviceReadError { device_id: i32 },

    #[error("Error write data to the PCIe40 device with id {device_id}")]
    DeviceWriteError { device_id: i32 },

    #[error("Invalid device name: {device_name}")]
    InvalidDeviceName { device_name: String },
}

impl Drop for PCIe40IdStream {
    fn drop(&mut self) {
        PCIe40Id::close(self);
    }
}

impl PCIe40IdStream {
    pub fn fpga_serial_number(&self) -> Result<i64, PCIe40IdStreamError> {
        let c_result = unsafe { p40_id_get_fpga(self.id_fd) };
        if c_result < 0 {
            Err(PCIe40IdStreamError::DeviceReadError {
                device_id: self.device_id,
            })?;
        }

        Ok(c_result)
    }

    pub fn device_name(&mut self) -> Result<String, PCIe40IdStreamError> {
        let mut buffer = vec![0u8; 9];

        let c_result = unsafe {
            p40_id_get_name(
                self.id_fd,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
            )
        };

        if c_result < 0 {
            return Err(PCIe40IdStreamError::DeviceReadError {
                device_id: self.device_id,
            });
        }

        let device_name = self.c_buffer_to_string(&buffer)?;

        Ok(device_name)
    }

    pub fn unique_device_name(&mut self) -> Result<String, PCIe40IdStreamError> {
        let mut buffer = vec![0u8; 9];

        let c_result = unsafe {
            p40_id_get_name_unique(
                self.id_fd,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
            )
        };

        if c_result < 0 {
            return Err(PCIe40IdStreamError::DeviceReadError {
                device_id: self.device_id,
            });
        }

        let unique_device_name = self.c_buffer_to_string(&buffer)?;

        Ok(unique_device_name)
    }

    pub fn set_name(&mut self, device_name: &str) -> Result<(), PCIe40IdStreamError> {
        let c_str_name =
            CString::new(device_name).or(Err(PCIe40IdStreamError::InvalidDeviceName {
                device_name: device_name.to_string(),
            }))?;

        let c_result = unsafe { p40_id_set_name(self.id_fd, c_str_name.as_ptr()) };

        if c_result < 0 {
            return Err(PCIe40IdStreamError::DeviceWriteError {
                device_id: self.device_id,
            });
        }

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

impl PCIe40IdStream {
    fn c_buffer_to_string(&self, buffer: &[u8]) -> Result<String, PCIe40IdStreamError> {
        let null_pos =
            buffer
                .iter()
                .position(|&c| c == 0)
                .ok_or(PCIe40IdStreamError::DeviceReadError {
                    device_id: self.device_id,
                })?;

        Ok(String::from_utf8_lossy(&buffer[0..null_pos]).to_string())
    }
}

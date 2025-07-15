use crate::pcie40::bindings::*;
use std::ffi::CString;
use std::ptr::null;
use thiserror::Error;
use tracing::{debug, info, instrument, trace, warn};

pub struct PCIe40IdManager {}

#[derive(Debug, Error)]
pub enum PCIe40IdManagerError {
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

impl PCIe40IdManager {
    #[instrument]
    pub fn id_endpoint_exists(device_id: i32) -> bool {
        debug!("Checking if device exists");

        trace!("Calling p40_id_exists({})", device_id);
        let c_result = unsafe { p40_id_exists(device_id) };
        trace!("p40_id_exists returned {}", c_result);

        c_result == 0
    }

    #[instrument(skip_all, fields(device_name = ?device_name.as_ref()))]
    pub fn find_id_by_name<T: AsRef<str>>(device_name: T) -> Result<i32, PCIe40IdManagerError> {
        let device_name = device_name.as_ref();

        let c_str_device_name = CString::new(device_name).map_err(|_| {
            warn!("Invalid device name: '{}'", device_name);
            PCIe40IdManagerError::InvalidDeviceName {
                device_name: device_name.to_string(),
            }
        })?;

        trace!("Calling p40_id_find(\"{}\")", device_name);
        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) };
        trace!("p40_id_find returned {}", device_id);

        if device_id < 0 {
            warn!("Device with name '{}' not found", device_name);
            Err(PCIe40IdManagerError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            })
        } else {
            debug!("Found device '{}' with ID {}", device_name, device_id);
            Ok(device_id)
        }
    }

    #[instrument(skip_all, fields(device_name = ?device_name.as_ref()))]
    pub fn find_all_ids_by_name<T: AsRef<str>>(
        device_name: T,
    ) -> Result<Vec<i32>, PCIe40IdManagerError> {
        let device_name = device_name.as_ref();

        debug!(
            "Looking up all device IDs for devices named '{}'",
            device_name
        );

        let c_str_device_name = CString::new(device_name).map_err(|_| {
            warn!("Invalid device name: '{}'", device_name);
            PCIe40IdManagerError::InvalidDeviceName {
                device_name: device_name.to_string(),
            }
        })?;

        trace!("Calling p40_id_find_all(\"{}\")", device_name);
        let mask = unsafe { p40_id_find_all(c_str_device_name.as_ptr()) };
        trace!("p40_id_find_all returned {:#x}", mask);

        let device_ids: Vec<i32> = (0..32)
            .filter(|&device_id| (mask & (1 << device_id)) != 0)
            .collect();

        debug!(
            "Found {} devices named '{}': {:?}",
            device_ids.len(),
            device_name,
            device_ids
        );

        Ok(device_ids)
    }

    #[instrument]
    pub fn find_all_ids() -> Result<Vec<i32>, PCIe40IdManagerError> {
        debug!("Looking up all device IDs",);

        trace!("Calling p40_id_find_all(\"\")");
        let mask = unsafe { p40_id_find_all(null()) };
        trace!("p40_id_find_all returned {:#x}", mask);

        let device_ids: Vec<i32> = (0..32)
            .filter(|&device_id| (mask & (1 << device_id)) != 0)
            .collect();

        debug!("Found {} devices: {:?}", device_ids.len(), device_ids);

        Ok(device_ids)
    }

    #[instrument(skip_all, fields(device_name = ?device_name.as_ref()))]
    pub fn open_by_device_name<T: AsRef<str>>(
        device_name: T,
    ) -> Result<PCIe40IdEndpoint, PCIe40IdManagerError> {
        let device_name = device_name.as_ref();

        debug!("Opening ID endpoint for device named '{}'", device_name);

        let c_str_device_name = CString::new(device_name).map_err(|_| {
            warn!("Invalid device name: '{}'", device_name);
            PCIe40IdManagerError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            }
        })?;

        trace!("Calling p40_id_find(\"{}\")", device_name);
        let device_id = unsafe { p40_id_find(c_str_device_name.as_ptr()) };
        trace!("p40_id_find returned {}", device_id);

        if device_id < 0 {
            warn!("Device with name '{}' not found", device_name);
            Err(PCIe40IdManagerError::DeviceNotFoundByName {
                device_name: device_name.to_string(),
            })?;
        };

        Self::open_by_device_id(device_id)
    }

    #[instrument]
    pub fn open_by_device_id(device_id: i32) -> Result<PCIe40IdEndpoint, PCIe40IdManagerError> {
        debug!("Opening ID endpoint for device with ID {}", device_id);

        trace!("Calling p40_id_exists({})", device_id);
        let exists = unsafe { p40_id_exists(device_id) };
        trace!("p40_id_exists returned {}", exists);

        if exists != 0 {
            warn!("Device with ID {} not found", device_id);
            Err(PCIe40IdManagerError::DeviceNotFoundById { device_id })?;
        }

        trace!("Calling p40_id_open({})", device_id);
        let id_fd = unsafe { p40_id_open(device_id) };
        trace!("p40_id_open returned {}", id_fd);

        if id_fd < 0 {
            warn!("Failed to open device with ID {}", device_id);
            Err(PCIe40IdManagerError::DeviceOpenFail { device_id })?;
        }

        debug!(
            "Successfully opened ID endpoint for device {} with fd {}",
            device_id, id_fd
        );
        Ok(PCIe40IdEndpoint { device_id, id_fd })
    }

    // Private function. Will be called by drop on PCIe40DeviceId
    fn close(id_endpoint: &PCIe40IdEndpoint) {
        debug!("Closing ID endpoint for device {}", id_endpoint.device_id);

        trace!("Calling p40_id_close({})", id_endpoint.id_fd);
        unsafe { p40_id_close(id_endpoint.id_fd) };

        debug!(
            "Closed ID endpoint for device {} with fd {}",
            id_endpoint.device_id, id_endpoint.id_fd
        );
    }
}

pub struct PCIe40IdEndpoint {
    device_id: i32,
    id_fd: i32,
}

#[derive(Debug, Error)]
pub enum PCIe40IdEndpointError {
    #[error("Error reading data from the PCIe40 device with id {device_id}")]
    DeviceReadError { device_id: i32 },

    #[error("Error write data to the PCIe40 device with id {device_id}")]
    DeviceWriteError { device_id: i32 },

    #[error("Invalid device name: {device_name}")]
    InvalidDeviceName { device_name: String },
}

impl Drop for PCIe40IdEndpoint {
    #[instrument(skip_all, fields(device_id = self.device_id))]
    fn drop(&mut self) {
        debug!(
            "Drop called on PCIe40IdEndpoint for device {}",
            self.device_id
        );
        PCIe40IdManager::close(self);
    }
}

impl PCIe40IdEndpoint {
    #[instrument(skip_all, fields(device_id = self.device_id))]
    pub fn fpga_serial_number(&self) -> Result<i64, PCIe40IdEndpointError> {
        debug!("Getting FPGA serial number for device {}", self.device_id);

        trace!("Calling p40_id_get_fpga({})", self.id_fd);
        let c_result = unsafe { p40_id_get_fpga(self.id_fd) };
        trace!("p40_id_get_fpga returned {}", c_result);

        if c_result < 0 {
            warn!(
                "Failed to get FPGA serial number for device {}",
                self.device_id
            );
            Err(PCIe40IdEndpointError::DeviceReadError {
                device_id: self.device_id,
            })?;
        }

        debug!(
            "FPGA serial number for device {}: {}",
            self.device_id, c_result
        );
        Ok(c_result)
    }

    #[instrument(skip_all, fields(device_id = self.device_id))]
    pub fn device_name(&mut self) -> Result<String, PCIe40IdEndpointError> {
        debug!("Getting device name for device {}", self.device_id);

        let mut buffer = vec![0u8; 9];

        trace!(
            "Calling p40_id_get_name({}, buffer, {})",
            self.id_fd,
            buffer.len()
        );
        let c_result = unsafe {
            p40_id_get_name(
                self.id_fd,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
            )
        };
        trace!("p40_id_get_name returned {}", c_result);

        if c_result < 0 {
            warn!("Failed to get device name for device {}", self.device_id);
            return Err(PCIe40IdEndpointError::DeviceReadError {
                device_id: self.device_id,
            });
        }

        let device_name = self.c_buffer_to_string(&buffer)?;
        debug!(
            "Device name for device {}: '{}'",
            self.device_id, device_name
        );

        Ok(device_name)
    }

    #[instrument(skip_all, fields(device_id = self.device_id))]
    pub fn unique_device_name(&mut self) -> Result<String, PCIe40IdEndpointError> {
        debug!("Getting unique device name for device {}", self.device_id);

        let mut buffer = vec![0u8; 9];

        trace!(
            "Calling p40_id_get_name_unique({}, buffer, {})",
            self.id_fd,
            buffer.len()
        );
        let c_result = unsafe {
            p40_id_get_name_unique(
                self.id_fd,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
            )
        };
        trace!("p40_id_get_name_unique returned {}", c_result);

        if c_result < 0 {
            warn!(
                "Failed to get unique device name for device {}",
                self.device_id
            );
            return Err(PCIe40IdEndpointError::DeviceReadError {
                device_id: self.device_id,
            });
        }

        let unique_device_name = self.c_buffer_to_string(&buffer)?;
        debug!(
            "Unique device name for device {}: '{}'",
            self.device_id, unique_device_name
        );

        Ok(unique_device_name)
    }

    #[instrument(skip_all, fields(device_id = self.device_id, name = ?device_name))]
    pub fn set_name(&mut self, device_name: &str) -> Result<(), PCIe40IdEndpointError> {
        debug!(
            "Setting device name for device {} to '{}'",
            self.device_id, device_name
        );

        let c_str_name = CString::new(device_name).map_err(|_| {
            warn!("Invalid device name: '{}'", device_name);
            PCIe40IdEndpointError::InvalidDeviceName {
                device_name: device_name.to_string(),
            }
        })?;

        trace!(
            "Calling p40_id_set_name({}, \"{}\")",
            self.id_fd, device_name
        );
        let c_result = unsafe { p40_id_set_name(self.id_fd, c_str_name.as_ptr()) };
        trace!("p40_id_set_name returned {}", c_result);

        if c_result < 0 {
            warn!(
                "Failed to set device name for device {} to '{}'",
                self.device_id, device_name
            );
            return Err(PCIe40IdEndpointError::DeviceWriteError {
                device_id: self.device_id,
            });
        }

        debug!(
            "Successfully set device name for device {} to '{}'",
            self.device_id, device_name
        );
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

impl PCIe40IdEndpoint {
    fn c_buffer_to_string(&self, buffer: &[u8]) -> Result<String, PCIe40IdEndpointError> {
        let null_pos = buffer.iter().position(|&c| c == 0).ok_or({
            PCIe40IdEndpointError::DeviceReadError {
                device_id: self.device_id,
            }
        })?;

        let result = String::from_utf8_lossy(&buffer[0..null_pos]).to_string();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::time::SystemTime;

    fn pcie40_device_name() -> String {
        std::env::var("HARDWARE_TESTS_DEVICE_NAME").unwrap_or_else(|_| {
            panic!(
                "Hardware test requires HARDWARE_TESTS_DEVICE_NAME environment variable.\n\
                To run this test first set HARDWARE_TESTS_DEVICE_NAME=<device-name>"
            )
        })
    }

    fn pcie40_device_name_non_existent() -> String {
        const MAX_RETRIES: usize = 5;

        for _ in 0..MAX_RETRIES {
            let mut hasher = DefaultHasher::new();
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .hash(&mut hasher);
            let non_existent_name = format!("NE_{:x}", hasher.finish());
            non_existent_name[0..std::cmp::min(non_existent_name.len(), 16)].to_string();

            if PCIe40IdManager::find_id_by_name(&non_existent_name).is_err() {
                return non_existent_name;
            }
        }

        panic!("Could not find a non-existent device name");
    }

    fn pcie40_device_id() -> i32 {
        PCIe40IdManager::find_id_by_name(pcie40_device_name()).unwrap()
    }

    fn pcie40_device_id_non_existent() -> i32 {
        match PCIe40IdManager::find_all_ids().unwrap().iter().max() {
            Some(id) => id + 1,
            None => 1,
        }
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_id_endpoint_exists() {
        let device_id = pcie40_device_id();
        let result = PCIe40IdManager::id_endpoint_exists(device_id);
        assert!(result);
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_id_endpoint_exists_non_existent() {
        let device_id = pcie40_device_id_non_existent();
        let result = PCIe40IdManager::id_endpoint_exists(device_id);
        assert!(!result);
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_find_id_by_name() {
        let device_name = pcie40_device_name();
        PCIe40IdManager::find_id_by_name(&device_name).unwrap();
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_find_id_by_name_non_existent() {
        let device_name = pcie40_device_name_non_existent();
        let result = PCIe40IdManager::find_id_by_name(&device_name);
        assert!(result.is_err());
        match result.err().unwrap() {
            PCIe40IdManagerError::DeviceNotFoundByName { .. } => {}
            _ => panic!(
                "Unexpected error; Should have raised a {:?}",
                PCIe40IdManagerError::DeviceNotFoundByName {
                    device_name: device_name.to_string()
                }
            ),
        }
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_find_all_ids_by_name() {
        let device_name = pcie40_device_name();
        let device_ids = PCIe40IdManager::find_all_ids_by_name(&device_name).unwrap();
        assert!(!device_ids.is_empty());

        // Verify each ID exists
        for id in &device_ids {
            assert!(PCIe40IdManager::id_endpoint_exists(*id));
        }
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_find_all_ids_by_name_non_existent() {
        let device_name = pcie40_device_name_non_existent();
        let device_ids = PCIe40IdManager::find_all_ids_by_name(&device_name).unwrap();
        assert!(device_ids.is_empty());
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_find_all_ids() {
        let device_ids = PCIe40IdManager::find_all_ids().unwrap();

        assert!(!device_ids.is_empty());

        for id in &device_ids {
            assert!(PCIe40IdManager::id_endpoint_exists(*id));
        }
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_open_by_device_name() {
        let device_name = pcie40_device_name();
        let endpoint = PCIe40IdManager::open_by_device_name(&device_name).unwrap();
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_open_by_device_name_non_existent() {
        let device_name = pcie40_device_name_non_existent();
        let result = PCIe40IdManager::open_by_device_name(&device_name);

        assert!(result.is_err());
        match result.err().unwrap() {
            PCIe40IdManagerError::DeviceNotFoundByName { .. } => {}
            _ => panic!("Unexpected error; Should have raised a DeviceNotFoundByName error"),
        }
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_open_by_device_id() {
        let device_id = pcie40_device_id();
        let endpoint = PCIe40IdManager::open_by_device_id(device_id).unwrap();
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_open_by_device_id_non_existent() {
        let device_id = pcie40_device_id_non_existent();
        let result = PCIe40IdManager::open_by_device_id(device_id);

        assert!(result.is_err());
        match result.err().unwrap() {
            PCIe40IdManagerError::DeviceNotFoundById { .. } => {}
            _ => panic!("Unexpected error; Should have raised a DeviceNotFoundById error"),
        }
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_fpga_serial_number() {
        let device_id = pcie40_device_id();
        let endpoint = PCIe40IdManager::open_by_device_id(device_id).unwrap();

        // No real way to test this unless the info can be known in an already tested way
        let serial = endpoint.fpga_serial_number().unwrap();
        assert!(serial >= 0, "FPGA serial number should be non-negative");
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_device_name() {
        let device_id = pcie40_device_id();
        let mut endpoint = PCIe40IdManager::open_by_device_id(device_id).unwrap();

        // No real way to test this unless the info can be known in an already tested way
        let unique_name = endpoint.unique_device_name().unwrap();
        assert!(
            !unique_name.is_empty(),
            "Unique device name should not be empty"
        );
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_unique_device_name() {
        let device_id = pcie40_device_id();
        let mut endpoint = PCIe40IdManager::open_by_device_id(device_id).unwrap();

        // No real way to test this unless the info can be known in an already tested way
        let unique_name = endpoint.unique_device_name().unwrap();
        assert!(
            !unique_name.is_empty(),
            "Unique device name should not be empty"
        );
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_set_name() {
        let device_id = pcie40_device_id();
        let mut endpoint = PCIe40IdManager::open_by_device_id(device_id).unwrap();

        // Get the original name so we can restore it later
        let original_name = endpoint.device_name().unwrap();

        // Create a test name
        let test_name = "TEST_DEV";

        // Set the new name
        endpoint.set_name(test_name).unwrap();

        // Verify the name was changed
        let updated_name = endpoint.device_name().unwrap();
        assert_eq!(
            updated_name, test_name,
            "Device name should have been updated"
        );

        // Restore the original name
        endpoint.set_name(&original_name).unwrap();

        // Verify the name was restored
        let restored_name = endpoint.device_name().unwrap();
        assert_eq!(
            restored_name, original_name,
            "Device name should have been restored"
        );
    }

    #[cfg(feature = "pcie40-hardware-tests")]
    #[test]
    fn test_with_hardware_set_name_invalid() {
        let device_id = pcie40_device_id();
        let mut endpoint = PCIe40IdManager::open_by_device_id(device_id).unwrap();

        // Try to set an invalid name with a null byte
        let result = endpoint.set_name("Test\0Name");

        match result.err().unwrap() {
            PCIe40IdEndpointError::InvalidDeviceName { .. } => {}
            _ => panic!("Unexpected error; Should have raised an InvalidDeviceName error"),
        }
    }
}

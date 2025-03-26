use crate::bindings::*;
use crate::zero_copy_reader::ZeroCopyReaderImpl;
use std::ffi::CString;
use std::io;


pub struct PCIe40ZeroCopyReaderImpl {}

impl PCIe40ZeroCopyReaderImpl {
    pub fn open_by_device_name(device_name: &str) -> Result<Self, PCIe40Error> {
        let c_string = CString::new(device_name).map_err(|_| {
            PCIe40Error::DeviceNotFound(device_name.to_string())
        })?;

        // Find device name
        let device_id = unsafe { p40_id_find(c_string.as_ptr()) };
        if device_id < 0 {
            Err(PCIe40Error::DeviceNotFound(device_name.to_string()))?;
        }

        Self::open_by_id(device_id)
    }

    pub fn open_by_id(device_id: i32) -> Result<Self, PCIe40Error> {
        let id_fd = unsafe { p40_id_open(device_id) };
        if id_fd < 0 {
            Err(PCIe40Error::DeviceOpenError(format!("{device_id}")))?;
        }

        // Open stream
        let stream_fd = unsafe { p40_stream_open(device_id, P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN) };
        if stream_fd < 0 {
            unsafe { p40_id_close(id_fd) };
        }
    }
}

impl ZeroCopyReaderImpl for PCIe40ZeroCopyReaderImpl {
    fn data(&self) -> &[u8] {
        todo!()
    }

    fn load_data(&mut self, num_bytes: usize) -> usize {
        todo!()
    }

    fn discard_data(&mut self, num_bytes: usize) -> usize {
        todo!()
    }
}

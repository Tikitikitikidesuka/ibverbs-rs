use std::ffi::CString;
use crate::bindings::p40_id_find;

mod bindings;

pub fn keo() {
    // Create a C-compatible string
    let device_name = CString::new("tdtel203_0").expect("CString::new failed");

    // Call the function (wrapped in unsafe because it's a foreign function)
    let device_id = unsafe {
        p40_id_find(device_name.as_ptr())
    };

    if device_id < 0 {
        println!("Error: Device not found");
    } else {
        println!("Device ID: {}", device_id);

        // Now you can use this ID with other functions
        // For example, to open the device
    }
}
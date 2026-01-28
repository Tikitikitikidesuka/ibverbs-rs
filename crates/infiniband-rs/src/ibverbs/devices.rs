use crate::ibverbs::context::Context;
use crate::ibverbs::global_unique_id::Guid;
use ibverbs_sys::*;
use std::ffi::CStr;
use std::io;
use std::marker::PhantomData;

pub fn open_device(name: impl AsRef<str>) -> io::Result<Context> {
    let name = name.as_ref();
    let devices = list_devices()?;
    let device = devices
        .iter()
        .find(|d| d.name() == Some(name))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("ibverbs device '{name}' not found"),
            )
        })?;
    device.open()
}

pub fn list_devices() -> io::Result<DeviceList> {
    let mut num_devices = 0i32;
    let devices_ptr = unsafe { ibv_get_device_list(&mut num_devices as *mut _) };

    if devices_ptr.is_null() {
        return Err(io::Error::last_os_error());
    }

    log::debug!("IbvDeviceList created");
    Ok(DeviceList {
        devices_ptr,
        num_devices: num_devices as usize,
    })
}

pub struct DeviceList {
    devices_ptr: *mut *mut ibv_device,
    num_devices: usize,
}

unsafe impl Sync for DeviceList {}
unsafe impl Send for DeviceList {}

// Leaking is not considered unsafe in Rust.
// The device list array can be leaked if the
// `DeviceList` gets `std::mem::forget` applied to it.
impl Drop for DeviceList {
    fn drop(&mut self) {
        log::debug!("IbvDeviceList dropped");
        unsafe { ibv_free_device_list(self.devices_ptr) };
    }
}

impl DeviceList {
    /// Returns an iterator over all found devices.
    pub fn iter(&self) -> DeviceListIter<'_> {
        DeviceListIter { list: self, i: 0 }
    }

    /// Returns the number of devices.
    pub fn len(&self) -> usize {
        self.num_devices as usize
    }

    /// Returns `true` if there are any devices.
    pub fn is_empty(&self) -> bool {
        self.num_devices == 0
    }

    /// Returns the device at the given `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<Device<'_>> {
        if index >= self.num_devices {
            return None;
        }

        Some(Device {
            device_ptr: unsafe { *self.devices_ptr.add(index) },
            _dev_list: PhantomData,
        })
    }
}

impl<'a> IntoIterator for &'a DeviceList {
    type Item = <DeviceListIter<'a> as Iterator>::Item;
    type IntoIter = DeviceListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        DeviceListIter { list: self, i: 0 }
    }
}

/// Iterator over a `DeviceList`.
pub struct DeviceListIter<'iter> {
    list: &'iter DeviceList,
    i: usize,
}

impl<'iter> Iterator for DeviceListIter<'iter> {
    type Item = Device<'iter>;
    fn next(&mut self) -> Option<Self::Item> {
        let opt_device = self.list.get(self.i);
        if opt_device.is_some() {
            self.i += 1;
        }
        opt_device
    }
}

impl std::fmt::Debug for DeviceList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub struct Device<'a> {
    device_ptr: *mut ibv_device,
    _dev_list: PhantomData<&'a DeviceList>,
}

unsafe impl<'a> Sync for Device<'a> {}
unsafe impl<'a> Send for Device<'a> {}

impl Device<'_> {
    pub(super) unsafe fn new(device_ptr: *mut ibv_device) -> Self {
        Self { device_ptr, _dev_list: PhantomData }
    }
}

impl Device<'_> {
    /// Opens an RMDA device and creates a context for further use.
    ///
    /// This context will later be used to query its resources or for creating resources.
    ///
    /// Unlike what the verb name suggests, it doesn't actually open the device. This device was
    /// opened by the kernel low-level driver and may be used by other user/kernel level code. This
    /// verb only opens a context to allow user level applications to use it.
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: `PORT_NUM` is invalid (from `ibv_query_port_attr`).
    ///  - `ENOMEM`: Out of memory (from `ibv_query_port_attr`).
    ///  - `EMFILE`: Too many files are opened by this process (from `ibv_query_gid`).
    ///  - Other: the device is not in `ACTIVE` or `ARMED` state.
    pub fn open(&self) -> io::Result<Context> {
        Context::with_device(self.device_ptr)
    }

    /// Returns a `&str` of the name, which is associated with this RDMA device.
    pub fn name(&self) -> Option<&str> {
        let name_ptr = unsafe { ibv_get_device_name(self.device_ptr) };
        if name_ptr.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(name_ptr).to_str().ok() }
        }
    }

    /// Returns the Global Unique IDentifier (GUID) of this RDMA device.
    ///
    /// # Errors
    ///  - `EMFILE`: Too many files are opened by this process.
    pub fn guid(&self) -> io::Result<Guid> {
        let guid_int = unsafe { ibv_get_device_guid(self.device_ptr) };
        let guid: Guid = guid_int.into();
        if guid.is_reserved() {
            Err(io::Error::last_os_error())
        } else {
            Ok(guid)
        }
    }
}

impl std::fmt::Debug for Device<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Format name
        let name_str = self.name().unwrap_or("<unknown>");

        // Format GUID
        let guid_str = match self.guid() {
            Ok(g) => format!("{:?}", g),
            Err(_) => "<error>".to_string(),
        };

        f.debug_struct("IbvDevice")
            .field("name", &name_str)
            .field("guid", &guid_str)
            .finish()
    }
}

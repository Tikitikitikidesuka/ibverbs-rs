use crate::ibverbs::context::IbvContext;
use crate::ibverbs::global_unique_id::Guid;
use ibverbs_sys::*;
use std::ffi::CStr;
use std::io;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub fn ibv_device_open(name: impl AsRef<str>) -> io::Result<IbvContext> {
    let name = name.as_ref();
    let devices = ibv_device_list()?;
    let device = devices
        .iter()
        .find(|d| d.name() == Some(name))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("InfiniBand device '{name}' not found"),
            )
        })?;
    device.open()
}

pub fn ibv_device_list() -> io::Result<IbvDeviceList> {
    let mut num_devices = 0i32;
    let devices_ptr = unsafe { ibv_get_device_list(&mut num_devices as *mut _) };

    if devices_ptr.is_null() {
        return Err(io::Error::last_os_error());
    }

    Ok(IbvDeviceList {
        devices_ptr,
        num_devices: num_devices as usize,
    })
}

pub struct IbvDeviceList {
    devices_ptr: *mut *mut ibv_device,
    num_devices: usize,
}

unsafe impl Sync for IbvDeviceList {}
unsafe impl Send for IbvDeviceList {}

// Leaking is not considered unsafe in Rust.
// The device list array can be leaked if the
// `DeviceList` gets `std::mem::forget` applied to it.
impl Drop for IbvDeviceList {
    fn drop(&mut self) {
        unsafe { ibv_free_device_list(self.devices_ptr) };
    }
}

impl IbvDeviceList {
    /// Returns an iterator over all found devices.
    pub fn iter(&self) -> IbDeviceListIter<'_> {
        IbDeviceListIter { list: self, i: 0 }
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
    pub fn get(&self, index: usize) -> Option<IbvDevice<'_>> {
        if index >= self.num_devices {
            return None;
        }

        Some(IbvDevice {
            device_ptr: unsafe { *self.devices_ptr.add(index) },
            _dev_list: PhantomData,
        })
    }
}

impl<'a> IntoIterator for &'a IbvDeviceList {
    type Item = <IbDeviceListIter<'a> as Iterator>::Item;
    type IntoIter = IbDeviceListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        IbDeviceListIter { list: self, i: 0 }
    }
}

/// Iterator over a `DeviceList`.
pub struct IbDeviceListIter<'iter> {
    list: &'iter IbvDeviceList,
    i: usize,
}

impl<'iter> Iterator for IbDeviceListIter<'iter> {
    type Item = IbvDevice<'iter>;
    fn next(&mut self) -> Option<Self::Item> {
        let opt_device = self.list.get(self.i);
        if opt_device.is_some() {
            self.i += 1;
        }
        opt_device
    }
}

impl std::fmt::Debug for IbvDeviceList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub struct IbvDevice<'devlist> {
    device_ptr: *mut ibv_device,
    _dev_list: PhantomData<&'devlist IbvDeviceList>,
}

unsafe impl<'devlist> Sync for IbvDevice<'devlist> {}
unsafe impl<'devlist> Send for IbvDevice<'devlist> {}

impl IbvDevice<'_> {
    pub(super) unsafe fn new(device_ptr: *mut ibv_device) -> Self {
        Self { device_ptr, _dev_list: PhantomData }
    }
}

impl IbvDevice<'_> {
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
    pub fn open(&self) -> io::Result<IbvContext> {
        IbvContext::with_device(self.device_ptr)
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

impl std::fmt::Debug for IbvDevice<'_> {
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

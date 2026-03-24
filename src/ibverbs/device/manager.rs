use crate::ibverbs::device::{Context, Guid};
use crate::ibverbs::error::{IbvError, IbvResult};
use ibverbs_sys::*;
use std::ffi::CStr;
use std::io;
use std::marker::PhantomData;

/// Convenience function to open an ibverbs RDMA device by name.
///
/// This function searches the list of available ibverbs devices for one whose
/// name matches `name` and opens a [`Context`] for it.
///
/// # Errors
///
/// * Returns [`IbvError::NotFound`] if no device with the given name exists.
/// * Propagates system errors if the device list cannot be retrieved.
pub fn open_device(name: impl AsRef<str>) -> IbvResult<Context> {
    let name = name.as_ref();
    let devices = list_devices()?;
    let device = devices
        .iter()
        .find(|d| d.name() == Some(name))
        .ok_or_else(|| IbvError::NotFound(format!("Device '{name}' not found")))?;
    device.open()
}

/// Returns a list of all available ibverbs RDMA devices.
///
/// The returned [`DeviceList`] owns the underlying device list allocated by
/// `libibverbs` and will free it automatically when dropped.
///
/// # Errors
///
/// Returns an [`IbvError`] if the underlying `ibv_get_device_list` call fails or returns `NULL`
/// with a non-zero `errno`.
pub fn list_devices() -> IbvResult<DeviceList> {
    let mut num_devices = 0i32;
    let devices_ptr = unsafe { ibv_get_device_list(&mut num_devices as *mut _) };

    if devices_ptr.is_null() {
        let errno = io::Error::last_os_error().raw_os_error().unwrap();
        // If errno is not zero, error fetching
        if errno != 0 {
            return Err(IbvError::from_errno_with_msg(
                errno,
                "Failed to list devices",
            ));
        }
    }

    log::debug!("DeviceList created");
    // ibv_get_device_list guarantees a non-negative count when the pointer is non-null
    #[allow(clippy::cast_sign_loss)]
    Ok(DeviceList {
        devices_ptr,
        num_devices: num_devices as usize,
    })
}

/// Owned list of available ibverbs RDMA devices.
///
/// This type wraps the device list returned by `ibv_get_device_list`.
/// The underlying resources are released automatically when the value
/// is dropped.
///
/// Individual devices can be accessed via iteration or indexing using
/// [`DeviceList::iter`] or [`DeviceList::get`].
pub struct DeviceList {
    devices_ptr: *mut *mut ibv_device,
    num_devices: usize,
}

/// SAFETY: libibverbs components are thread safe.
unsafe impl Sync for DeviceList {}
/// SAFETY: libibverbs components are thread safe.
unsafe impl Send for DeviceList {}

impl Drop for DeviceList {
    fn drop(&mut self) {
        if !self.devices_ptr.is_null() {
            log::debug!("DeviceList dropped");
            // SAFETY: self.devices_ptr is guaranteed to be a valid pointer returned
            // by ibv_get_device_list or null (checked above).
            unsafe { ibv_free_device_list(self.devices_ptr) };
        }
    }
}

impl DeviceList {
    /// Returns an iterator over all available devices.
    pub fn iter(&self) -> DeviceListIter<'_> {
        DeviceListIter { list: self, i: 0 }
    }

    /// Returns the number of available devices.
    pub fn len(&self) -> usize {
        self.num_devices
    }

    /// Returns `true` if no devices are available.
    pub fn is_empty(&self) -> bool {
        self.num_devices == 0
    }

    /// Returns a reference to the device at the given index.
    ///
    /// Returns `None` if the index is out of bounds. The returned [`Device`]
    /// is bound to the lifetime of this list.
    pub fn get(&self, index: usize) -> Option<Device<'_>> {
        if index >= self.num_devices {
            return None;
        }

        // SAFETY: Verified `index` is within `num_devices` and `devices_ptr`
        // is an array of pointers to `ibv_device` structs.
        Some(unsafe { Device::from_ptr(*self.devices_ptr.add(index)) })
    }
}

impl<'a> IntoIterator for &'a DeviceList {
    type Item = <DeviceListIter<'a> as Iterator>::Item;
    type IntoIter = DeviceListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        DeviceListIter { list: self, i: 0 }
    }
}

/// An iterator over the devices in a [`DeviceList`].
///
/// This struct is created by the [`iter`](DeviceList::iter) method on [`DeviceList`].
/// Each item yielded is a [`Device`] that borrows from the parent list.
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

/// A reference to an RDMA device.
///
/// This type represents a borrowed handle to an RDMA device.
/// It can be obtained from a [`DeviceList`] or a [`Context`].
///
/// The reference is valid only as long as the source object ([`DeviceList`] or [`Context`])
/// remains alive.
///
/// To perform operations on the device, you must first [`open`](Device::open) it to obtain a [`Context`].
pub struct Device<'a> {
    pub(super) device_ptr: *mut ibv_device,
    _dev_list: PhantomData<&'a DeviceList>,
}

/// SAFETY: libibverbs components are thread safe.
unsafe impl Sync for Device<'_> {}
/// SAFETY: libibverbs components are thread safe.
unsafe impl Send for Device<'_> {}

impl Device<'_> {
    /// Opens a context for this RDMA device.
    ///
    /// The resulting [`Context`] is the primary object used for allocating resources
    /// (PDs, QPs, CQs) and managing the device.
    ///
    /// # Errors
    ///
    /// Returns an error if the device cannot be opened (e.g., due to permission issues
    /// or resource exhaustion).
    pub fn open(&self) -> IbvResult<Context> {
        Context::from_device(self)
    }

    /// Returns the system name of the device (e.g., "mlx5_0").
    ///
    /// Returns `None` if the name cannot be retrieved or is not valid UTF-8.
    pub fn name(&self) -> Option<&str> {
        // SAFETY: ibv_get_device_name returns a pointer to a static string managed
        // by libibverbs. It is valid as long as the device ref is valid.
        let name_ptr = unsafe { ibv_get_device_name(self.device_ptr) };
        if name_ptr.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(name_ptr).to_str().ok() }
        }
    }

    /// Returns the Global Unique Identifier (GUID) of this RDMA device.
    ///
    /// # Errors
    ///
    /// Returns an error if the GUID is reserved (invalid) or cannot be read.
    pub fn guid(&self) -> IbvResult<Guid> {
        let guid_int = unsafe { ibv_get_device_guid(self.device_ptr) };
        let guid: Guid = guid_int.into();
        if guid.is_reserved() {
            Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "GUID is reserved or invalid",
            ))
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

        f.debug_struct("Device")
            .field("name", &name_str)
            .field("guid", &guid_str)
            .finish()
    }
}

impl Device<'_> {
    /// Wraps a raw `ibv_device` pointer into a `DeviceRef`.
    ///
    /// # Safety
    ///
    /// * `device_ptr` must be a valid pointer obtained from `ibv_get_device_list` or an active `ibv_context`.
    /// * The lifetime of the returned `DeviceRef` must not outlive the object that owns the pointer.
    pub(super) unsafe fn from_ptr(device_ptr: *mut ibv_device) -> Self {
        Self {
            device_ptr,
            _dev_list: PhantomData,
        }
    }
}

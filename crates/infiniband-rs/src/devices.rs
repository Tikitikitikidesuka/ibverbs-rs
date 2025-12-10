use crate::context::IbvContext;
use ibverbs_sys::*;
use std::ffi::CStr;
use std::io;

pub fn ibv_device_list() -> io::Result<IbvDeviceList> {
    let mut n = 0i32;
    let devices = unsafe { ibv_get_device_list(&mut n as *mut _) };

    if devices.is_null() {
        return Err(io::Error::last_os_error());
    }

    let devices = unsafe {
        use std::slice;
        slice::from_raw_parts_mut(devices, n as usize)
    };

    Ok(IbvDeviceList(devices))
}

pub struct IbvDeviceList(&'static mut [*mut ibv_device]);

unsafe impl Sync for IbvDeviceList {}
unsafe impl Send for IbvDeviceList {}

// Leaking is not considered unsafe in Rust.
// The device list array can be leaked if the
// `DeviceList` gets `std::mem::forget` applied to it.
impl Drop for IbvDeviceList {
    fn drop(&mut self) {
        unsafe { ibv_free_device_list(self.0.as_mut_ptr()) };
    }
}

impl IbvDeviceList {
    /// Returns an iterator over all found devices.
    pub fn iter(&self) -> IbDeviceListIter<'_> {
        IbDeviceListIter { list: self, i: 0 }
    }

    /// Returns the number of devices.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if there are any devices.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the device at the given `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<IbvDevice<'_>> {
        self.0.get(index).map(|d| IbvDevice(d))
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
        let e = self.list.0.get(self.i);
        if e.is_some() {
            self.i += 1;
        }
        e.map(|e| IbvDevice(e))
    }
}

impl std::fmt::Debug for IbvDeviceList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub struct IbvDevice<'devlist>(pub(super) &'devlist *mut ibv_device);

unsafe impl<'devlist> Sync for IbvDevice<'devlist> {}
unsafe impl<'devlist> Send for IbvDevice<'devlist> {}

impl<'devlist> IbvDevice<'devlist> {
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
        IbvContext::with_device(*self.0)
    }

    /// Returns a `&str` of the name, which is associated with this RDMA device.
    pub fn name(&self) -> Option<&'devlist str> {
        let name_ptr = unsafe { ibv_get_device_name(*self.0) };
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
        let guid_int = unsafe { ibv_get_device_guid(*self.0) };
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

/// A Global unique identifier for ibv.
///
/// This struct acts as a rust wrapper for GUID value represented as `__be64` in
/// libibverbs. We introduce this struct, because u64 is stored in host
/// endianness, whereas ibverbs stores GUID in network order (big endian).
#[derive(Default, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Guid {
    raw: [u8; 8],
}

impl Guid {
    /// Upper 24 bits of the GUID are OUI (Organizationally Unique Identifier,
    /// http://standards-oui.ieee.org/oui/oui.txt). The function returns OUI as
    /// a 24-bit number inside a u32.
    pub fn oui(&self) -> u32 {
        let padded = [0, self.raw[0], self.raw[1], self.raw[2]];
        u32::from_be_bytes(padded)
    }

    /// Returns `true` if this GUID is all zeroes, which is considered reserved.
    pub fn is_reserved(&self) -> bool {
        self.raw == [0; 8]
    }
}

impl From<u64> for Guid {
    fn from(guid: u64) -> Self {
        Self {
            raw: guid.to_be_bytes(),
        }
    }
}

impl From<Guid> for u64 {
    fn from(guid: Guid) -> Self {
        u64::from_be_bytes(guid.raw)
    }
}

impl AsRef<__be64> for Guid {
    fn as_ref(&self) -> &__be64 {
        unsafe { &*self.raw.as_ptr().cast::<__be64>() }
    }
}

impl std::fmt::Debug for Guid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.raw[0],
            self.raw[1],
            self.raw[2],
            self.raw[3],
            self.raw[4],
            self.raw[5],
            self.raw[6],
            self.raw[7]
        )
    }
}

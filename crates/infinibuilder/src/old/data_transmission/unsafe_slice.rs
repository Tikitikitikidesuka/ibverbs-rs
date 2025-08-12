use std::ptr::NonNull;

pub struct UnsafeSlice {
    ptr: NonNull<[u8]>,
}

impl UnsafeSlice {
    pub unsafe fn new(slice: &[u8]) -> Self {
        Self {
            ptr: NonNull::from(slice),
        }
    }
}

impl AsMut<[u8]> for UnsafeSlice {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { self.ptr.as_mut() }
    }
}

impl AsRef<[u8]> for UnsafeSlice {
    fn as_ref(&self) -> &[u8] {
        unsafe { self.ptr.as_ref() }
    }
}

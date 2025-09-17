use std::ptr::NonNull;

pub struct UnsafeSlice<T> {
    ptr: NonNull<[T]>,
}

impl<T> UnsafeSlice<T> {
    pub unsafe fn new(slice: &[T]) -> Self {
        Self {
            ptr: NonNull::from(slice),
        }
    }
}

impl<T> AsMut<[T]> for UnsafeSlice<T> {
    fn as_mut(&mut self) -> &mut [T] {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T> AsRef<[T]> for UnsafeSlice<T> {
    fn as_ref(&self) -> &[T] {
        unsafe { self.ptr.as_ref() }
    }
}

use std::cell::UnsafeCell;
/// This might actually not do exactly what I think it does.
/// Rust makes optimizations based on the assumption that &mut []
/// are unique references to memory (no aliasing) which this type
/// violates by implementing AsMut<[T]>... Could be UB.
/// UnsafeCell avoids this assumption on the compiler
/// TODO: INVESTIGATE THIS

use std::ptr::NonNull;

pub struct UnsafeSlice<T> {
    ptr: NonNull<[T]>,
}

impl<T> UnsafeSlice<T> {
    pub fn new(slice: &[T]) -> Self {
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

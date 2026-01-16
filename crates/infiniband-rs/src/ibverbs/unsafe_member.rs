use std::ops::Deref;

/// A simple wrapper struct to make assigning/writing a member unsafe.
///
/// This can be useful if you want to ensure that an invariant cannot be violated in safe code.
///
/// Note that this wrapper implements `Copy`, `Clone` if possible.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)] // importantly no Default
pub struct UnsafeMember<T>(T);

impl<T> UnsafeMember<T> {
    /// # Safety
    /// See requirements / invariants at member declaration.
    pub unsafe fn new(value: T) -> Self {
        UnsafeMember(value)
    }

    pub fn get(&self) -> &T {
        &self.0
    }

    /// # Safety
    /// See requirements / invariants at member declaration.
    pub unsafe fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for UnsafeMember<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

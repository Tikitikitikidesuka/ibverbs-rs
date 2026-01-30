use ibverbs_sys::ibv_access_flags;

#[derive(Debug, Copy, Clone)]
pub struct AccessFlags(u32);

impl AccessFlags {
    /// New access flags with no flags set
    pub fn new() -> Self {
        Self(0)
    }

    pub fn with_local_write(mut self) -> Self {
        self.0 |= ibv_access_flags::IBV_ACCESS_LOCAL_WRITE.0;
        self
    }

    pub fn with_remote_read(mut self) -> Self {
        self.0 |= ibv_access_flags::IBV_ACCESS_REMOTE_READ.0;
        self
    }

    pub fn with_remote_write(mut self) -> Self {
        self.0 |= ibv_access_flags::IBV_ACCESS_REMOTE_WRITE.0;
        self
    }

    pub fn code(&self) -> u32 {
        self.0
    }
}

impl Default for AccessFlags {
    fn default() -> AccessFlags {
        AccessFlags::new().with_local_write()
    }
}

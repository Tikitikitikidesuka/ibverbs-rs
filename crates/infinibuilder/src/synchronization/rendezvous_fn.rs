use crate::rdma_traits::{RdmaSync, SyncState, Timeout};
use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub struct NoTimeoutSyncFn;

#[derive(Debug, Copy, Clone)]
pub struct TimeoutSyncFn {
    pub(super) timeout: Duration,
}

pub trait SyncFn {
    fn synchronize<T: RdmaSync>(&self, conn: &mut T) -> Result<T::Result, Timeout>;
}

impl SyncFn for NoTimeoutSyncFn {
    #[inline(always)]
    fn synchronize<T: RdmaSync>(&self, conn: &mut T) -> Result<T::Result, Timeout> {
        Ok(conn.synchronize())
    }
}

impl SyncFn for TimeoutSyncFn {
    #[inline(always)]
    fn synchronize<T: RdmaSync>(&self, conn: &mut T) -> Result<T::Result, Timeout> {
        conn.synchronize_with_timeout(self.timeout)
    }
}

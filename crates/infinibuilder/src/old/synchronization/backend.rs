use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbBSyncError {
}

pub trait IbBSyncBackend {
    fn spin_poll_sync() -> IbBSyncError;
}

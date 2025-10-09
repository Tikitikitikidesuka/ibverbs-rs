use thiserror::Error;

//pub mod binary;
pub mod centralized;
pub mod dissemination;
//mod rendezvous_fn;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Self node not in the sync group")]
    SelfNotInSyncGroupError,
    #[error("Sync group is empty")]
    EmptyGroup,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

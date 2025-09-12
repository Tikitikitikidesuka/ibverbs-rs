use crate::synchronization::{SyncComponent, UnconnectedSyncComponent};
use crate::transfer::common::TransferError;
use crate::transfer::receiver::ReceiverTransfer;
use crate::transfer::sender::SenderTransfer;
use std::ops::RangeBounds;

pub struct SenderSyncedTransfer<Sync: SyncComponent> {
    sync: Sync,
    transfer: SenderTransfer,
}

impl<Sync: SyncComponent> SenderSyncedTransfer<Sync> {
    pub fn new() {

    }

    pub fn synced_send(
        &mut self,
        receiver_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<(), TransferError> {
        self.sync.wait_barrier()?;
        self.transfer.wait_send(receiver_idx, memory_range)?;
        Ok(())
    }
}

pub struct ReceiverSyncedTransfer<Sync: SyncComponent> {
    sync: Sync,
    transfer: ReceiverTransfer,
}

impl<Sync: SyncComponent> ReceiverSyncedTransfer<Sync> {
    pub fn synced_receive(
        &mut self,
        sender_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<(), TransferError> {
        let wr = self.transfer.post_receive(sender_idx, memory_range)?;
        self.sync.wait_barrier()?;
        wr.wait()?;
        Ok(())
    }
}

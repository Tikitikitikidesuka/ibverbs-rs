use std::ops::RangeBounds;
use std::time::{Duration, Instant};

pub trait RdmaSendRecv {
    unsafe fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static>;

    unsafe fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static>;
}

/*
New Sync:
- increase_epoch() -> usize // Increases the local epoch and notifies the peer
- wait_epoch(epoch: usize) // Waits for epoch `epoch` from the peer
- sync() // Waits for the peer to be on the same epoch

This version allows non blocking notifying. For example, when we want to synchronize a send/recv.
The receiver has to first post the receive and then notify the sender, so the sender needs to wait
for this notification, but the receiver does not need to wait for anything. With the previous rendezvous
implementation, both the receiver and the sender would have to wait for each-others message.
This way, the receiver posts the recv and then sends calls `increase_epoch`. This will notify the
sender. The sender will have to call `wait_epoch`.

Actually no. We just need a sync send/recv interface specialized for this.
We have three counters, post_tokens, issued_posts, issued_recvs. All start at zero.
- Whenever post_recv is called, issued_recvs is increased by one. Then its value is rdma written to the peer's
post_tokens.
- Whenever post_send is called, post_tokens is checked, if it is greater than issued_posts, issued_posts is increased by one
and the send is executed.

Run example:
t0 -> A
- post_tokens:  0
- issued_posts: 0
- issued_recvs: 0
t0 -> B
- post_tokens:  0
- issued_posts: 0
- issued_recvs: 0

t1 -> A post_send(post_tokens not greater than issued_posts) X
- post_tokens:  0
- issued_posts: 0
- issued_recvs: 0
t1 -> B
- post_tokens:  0
- issued_posts: 0
- issued_recvs: 0

t2 -> A
- post_tokens:  1
- issued_posts: 0
- issued_recvs: 0
t2 -> B post_recv O
- post_tokens:  0
- issued_posts: 0
- issued_recvs: 1

t3 -> A post_send O
- post_tokens:  1
- issued_posts: 1
- issued_recvs: 0
t3 -> B
- post_tokens:  0
- issued_posts: 0
- issued_recvs: 1

This way, there is only one rdma transaction per send recv instead of two :D.
*/

pub trait RdmaSyncSendRecv {
    unsafe fn post_sync_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static>;

    unsafe fn post_sync_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static>;
}

pub trait RdmaReadWrite {
    unsafe fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static>;

    unsafe fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static>;
}

/// Represents the synchronization state a connected peer implementing RdmaSync is in:
/// - `Synchronized`: Both peers have signaled the same number of barriers.
/// - `Ahead`: Local peer has signaled one more barrier than the remote.
/// - `Behind`: The remote peer has signaled one more barrier than the local.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub enum SyncState {
    Ahead,
    Synced,
    Behind,
}

/// Result of a timeout during a `RdmaSync::synchronize` call.
#[derive(Debug, Copy, Clone)]
pub struct Timeout;

/// Used for peer to peer synchronization through RDMA.
/// The API defines barriers as a point in code that both peers must go through.
/// When a peer signals a barrier, it cannot signal another until the remote does.
/// This can be used for waiting before executing sections that depend on the peer's previous work.
/// However, instead of just forcing a peer to wait when signaling a barrier,
/// one can signal to the peer that it is ready to pass a barrier without blocking
/// and continue working on other computations.
///
/// A peer can be in either of three states:
/// - Synchronized: Both peers have signaled the same number of barriers.
/// - Ahead: Local peer has signaled one more barrier than the remote.
/// - Behind: The remote peer has signaled one more barrier than the local.
pub trait RdmaSync {
    /// Returns the state of the synchronization which can be:
    /// - `Synchronized`: Both peers have signaled the same number of barriers.
    /// - `Ahead`: Local peer has signaled one more barrier than the remote.
    /// - `Behind`: The remote peer has signaled one more barrier than the local.
    fn sync_state(&self) -> SyncState;

    /// Signals a barrier to the peer, indicating it that the local is ready to pass it.
    /// If `signal_peer` has already been called before on the same barrier, meaning the peer
    /// has not signaled that barrier yet, this function does nothing and returns None.
    /// Otherwise, it returns `Some(Self::Result)` where th inner data is the result of the communication,
    /// which may or may not fail depending on the implementation.
    fn signal_peer(&mut self) -> Option<std::io::Result<()>>;

    /// Moves the local peer to the `Synchronized` state this is done differently depending on the
    /// current sync state:
    /// - If `Synchronized`, the function does nothing.
    /// - If `Ahead`, the function must wait until the state is `Synchronized`.
    /// - If `Behind`, the function signals a barrier to the remote peer.
    /// The result of this function is always reaching the `Synchronized` state.
    fn synchronize(&mut self) -> std::io::Result<()> {
        match self.sync_state() {
            SyncState::Synced => Ok(()),
            SyncState::Ahead => {
                spin_poll(|| self.sync_state() == SyncState::Synced);
                Ok(())
            }
            SyncState::Behind => self.signal_peer().unwrap(),
        }
    }

    /// Same as `synchronize` but with a timeout to avoid blocking forever.
    /// - If `Synchronized`, the function does nothing.
    /// - If `Ahead`, the function must wait until the state is `Synchronized`.
    /// - If `Behind`, the function signals a barrier to the remote peer.
    /// This function does not guarantee reaching the `Synchronized`state since it might
    /// timeout before achieving that its goal.
    fn synchronize_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<std::io::Result<()>, Timeout> {
        match self.sync_state() {
            SyncState::Synced => Ok(Ok(())),
            SyncState::Ahead => {
                spin_poll_with_timeout(|| self.sync_state() == SyncState::Synced, timeout)?;
                Ok(Ok(()))
            }
            SyncState::Behind => Ok(self.signal_peer().unwrap()),
        }
    }

    /// Waits until the status sync is `Behind` meaning the peer has issued a new barrier.
    fn wait_for_new_barrier(&self) {
        spin_poll(|| self.sync_state() == SyncState::Behind);
    }

    /// Same as `wait_for_new_barrier` but with a timeout to avoid blocking forever.
    fn wait_for_new_barrier_with_timeout(&self, timeout: Duration) -> Result<(), Timeout> {
        spin_poll_with_timeout(|| self.sync_state() == SyncState::Behind, timeout)
    }
}

pub fn spin_poll(cond: impl Fn() -> (bool)) {
    while !cond() {}
}

pub fn spin_poll_with_timeout(cond: impl Fn() -> (bool), timeout: Duration) -> Result<(), Timeout> {
    let start_time = Instant::now();

    while !cond() {
        if start_time.elapsed() > timeout {
            return Err(Timeout);
        }
    }

    Ok(())
}

pub trait WorkRequest {
    fn poll(&mut self) -> std::io::Result<Option<WorkCompletion>>;
    fn wait(self) -> std::io::Result<WorkCompletion>;
    fn wait_timeout(self, timeout: Duration) -> std::io::Result<WorkCompletion>;
}

#[derive(Debug, Copy, Clone)]
pub struct WorkCompletion {
    pub len: usize,
    pub imm_data: Option<u32>,
}

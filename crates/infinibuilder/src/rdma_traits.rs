use std::ops::RangeBounds;
use std::time::Duration;

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

pub trait RdmaRendezvous {
    fn is_peer_waiting(&self) -> bool;
    fn wait_for_peer_signal(&self) -> std::io::Result<()>;
    fn wait_for_peer_signal_timeout(&self, timeout: Duration) -> std::io::Result<()>;
    fn rendezvous(&mut self) -> std::io::Result<()>;
    fn rendezvous_timeout(&mut self, timeout: Duration) -> std::io::Result<()>;
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

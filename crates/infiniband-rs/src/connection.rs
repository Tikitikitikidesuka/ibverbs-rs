use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::unsafe_member::UnsafeMember;

pub type Result<T = (), E = io::Error> = std::result::Result<T, E>;

pub struct IbConnection {
    //mrs: HashMap<String, Mr>,
    //remote_mrs: HashMap<String, RemoteMr>,
}

impl IbConnection {
    pub fn new() {
        todo!()
    }

    pub fn register_mr(&mut self, name: impl Into<String>, region: *mut [u8]) -> Result {
        //self.inner.register_mr(name, region)
        todo!()
    }

    pub fn register_dmabuf_mr(
        &mut self,
        name: impl Into<String>,
        fd: i32,
        region: *mut [u8],
    ) -> io::Result<()> {
        todo!()
    }

    // Safety: When sharing an mr, it is exposed to be mutated remotely
    // by the peer at any point. It is the user's responsibility to ensure
    // a protocol to comply with Rust's memory safety guarantees.
    pub unsafe fn share_mr(&mut self, name: impl AsRef<str>) -> Result {
        //self.inner.share_mr(mr)
        todo!()
    }

    pub fn accept_shared_mr(&mut self) -> Result<RemoteMr> {
        //self.inner.accept_shared_mr()
        todo!()
    }

    pub fn remote_mr(&mut self, name: impl AsRef<str>) -> Option<RemoteMr> {
        //self.inner.remote_mr(name)
        todo!()
    }

    pub fn deregister_mr(&mut self, name: impl AsRef<str>) -> Result {
        //self.inner.deregister_mr(mr)
        todo!()
    }

    /// This method allows to safely send and receive data in a subscope, similar to [`std::thread::scope`].
    ///
    /// Scoping solves the problem of users being able to access memory regions scheduled for
    /// an RDMA operation before it is complete. If the methods to send, receive, read, write, etc,
    /// were in this class, the returned work requests could be dropped before the operation finished.
    /// If the work requests implemented a Drop trait to poll before being dropped, the user could
    /// forget them beforehand safely anyway, and so access the memory before the operation finished.
    /// The solution for this, as proposed by Jonatan, is to use the same scoping method as the one used
    /// for scoped treads. In this way, the created work requests have a well defined lifetime —that of
    /// the scope— and are stored in a private structure such that the user cannot forget them to avoid polling.
    /// If they have not been polled at the end of the scope, they will be polled automatically.
    // pub fn scope<'env, F, R>(&mut self, f: F) -> Result<R>
    // where
    //     F: for<'scope> FnOnce(&'scope mut IbConnectionScope<'scope, 'env>) -> Result<R>,
    // {
    //     todo!()
    // }

    // todo do we want to return the poll duration / number of local bytes written?
    // todo do these functions assert that the slice length maches exact? how would we do that?
    pub fn send_polled<'a>(&mut self, data: &'a [u8]) -> Result<()> {
        todo!()
    }

    pub fn receive_polled<'a>(&mut self, data: &'a mut [u8]) -> Result<()> {
        todo!()
    }

    pub fn send_paralell<'a>(&mut self, data: impl Iterator<Item = &'a [u8]>) -> Result<()> {
        todo!()
    }

    pub fn receive_paralell<'a>(&mut self, data: impl Iterator<Item = &'a mut [u8]>) -> Result<()> {
        todo!()
    }

    /// # Safety
    /// The caller must ensure that the work request is polled to completion before the end of `'a`.
    unsafe fn send_unpolled<'a>(&mut self, data: &'a [u8]) -> Result<WorkRequest<'a>> {
        todo!()
    }

    /// # Safety
    /// The caller must ensure that the work request is polled to completion before the end of `'a`.
    unsafe fn receive_unpolled<'a>(&mut self, data: &'a mut [u8]) -> Result<WorkRequest<'a>> {
        todo!()
    }
}

// pub struct IbConnectionScope<'scope, 'env: 'scope> {
//     inner: &'scope mut IbConnection,
//     wrs: Vec<WorkRequest<'scope>>,
//     cq: Rc<RefCell<CachedCompletionQueue>>,
//     // for invariance of lifetimes, see std::thread::scope
//     scope: PhantomData<&'scope mut &'scope ()>,
//     env: PhantomData<&'env mut &'env ()>,
// }

// impl<'scope, 'env> From<WorkRequest<'env>> for ScopedWorkRequest<'scope, 'env> {
//     fn from(value: WorkRequest<'env>) -> Self {
//         ScopedWorkRequest {
//             inner: value,
//             env: PhantomData,
//         }
//     }
// }

// impl<'scope, 'env> IbConnectionScope<'scope, 'env> {
//     // The slice cannot be used again until the work request is consumed,
//     // so no overlapping sends can be done concurrently
//     pub fn post_send(
//         &'scope mut self,
//         slice: &'env [u8],
//     ) -> Result<ScopedWorkRequest<'scope, 'env>> {
//         // TODO: Post to infiniband hardware

//         let wr = WorkRequest {
//             wr_id: 0, // Whatever id it is
//             cq: self.cq.clone(),
//             _data_lifetime: PhantomData,
//         };

//         self.wrs.push(wr.clone());

//         Ok(wr.into())
//     }

//     // The slice cannot be used again until the work request is consumed,
//     // so no overlapping receives can be done concurrently
//     pub fn post_receive(
//         &'scope mut self,
//         slice: &'env mut [u8],
//     ) -> Result<ScopedWorkRequest<'scope, 'env>> {
//         // TODO: Post to infiniband hardware

//         let wr = WorkRequest {
//             wr_id: 0, // Whatever id it is
//             cq: self.cq.clone(),
//             _data_lifetime: PhantomData,
//         };

//         self.wrs.push(wr.clone());

//         Ok(wr.into())
//     }

//     // Safety: The data at the remote memory region might be modified while the read is done.
//     // It is the user's responsibility to ensure it is stable while the read is in progress.
//     pub unsafe fn post_read(
//         &'scope mut self,
//         from_slice: &'env RemoteMrSlice,
//         into_slice: &'env mut [u8],
//     ) -> Result<ScopedWorkRequest<'scope, 'env>> {
//         // TODO: Post to infiniband hardware

//         let wr = WorkRequest {
//             wr_id: 0, // Whatever id it is
//             cq: self.cq.clone(),
//             _data_lifetime: PhantomData,
//         };

//         self.wrs.push(wr.clone());

//         Ok(wr.into())
//     }

//     // Safety: The data at the remote memory region will be modified regardless of its mutability
//     // status. It is the user's responsibility to ensure no use of the memory is being done concurrently.
//     pub unsafe fn post_write(
//         &'scope mut self,
//         from_slice: &'env [u8],
//         into_slice: &'env RemoteMrSlice,
//     ) -> Result<ScopedWorkRequest<'scope, 'env>> {
//         // TODO: Post to infiniband hardware

//         let wr = WorkRequest {
//             wr_id: 0, // Whatever id it is
//             cq: self.cq.clone(),
//             _data_lifetime: PhantomData,
//         };

//         self.wrs.push(wr.clone());

//         Ok(wr.into())
//     }
// }

pub struct CachedCompletionQueue;

// pub struct ScopedWorkRequest<'scope, 'env: 'scope> {
//     inner: WorkRequest<'env>,
//     env: PhantomData<&'scope mut &'scope ()>,
// }

#[derive(Clone)]
pub struct WorkRequest<'env> {
    wr_id: u64,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    /// SAFETY INVARIANT: The lifetime of the data must be the same as the lifetime of the work request.
    _data_lifetime: UnsafeMember<PhantomData<&'env [u8]>>,
}

pub struct WorkCompletion;

type WorkRequestStatus = Option<WorkCompletionResult>;
type WorkCompletionResult = Result<WorkCompletion>;

impl WorkRequest<'_> {
    // Returns None if the work request is not yet complete.
    // Otherwise returns the completion status of the work request.
    // The completion status can be Ok(WorkCompletion) or Err(io::Error).
    pub fn poll(&mut self) -> WorkRequestStatus {
        // TODO: Poll completion queue and manage cache
        Some(Ok(WorkCompletion))
    }

    // Polls the work request until it is complete or the timeout is reached.
    // Timeout is represented as None ouptut.
    fn spin_poll(&mut self, timeout: Duration) -> Option<WorkCompletionResult> {
        const ELAPSED_CHECK_ITERS: usize = 1024;
        self.spin_poll_batched::<ELAPSED_CHECK_ITERS>(timeout)
    }

    // Polls the work request until it is complete or the timeout is reached.
    // Timeout is represented as None ouptut.
    // To avoid getting time every iteration,
    // only check timeout every ELAPSED_CHECK_ITERS iterations.
    // For performance, this should be a power of 2 (for the modulus operation).
    fn spin_poll_batched<const TIMEOUT_CHECK_ITERS: usize>(
        &mut self,
        timeout: Duration,
    ) -> Option<WorkCompletionResult> {
        let start_time = Instant::now();

        let mut poll_iter = 0;
        loop {
            if let Some(wc_result) = self.poll() {
                return Some(wc_result);
            }

            if poll_iter % TIMEOUT_CHECK_ITERS == 0 {
                if start_time.elapsed() > timeout {
                    return None;
                }
            }

            poll_iter += 1;
        }
    }
}

// Safety: memory of an mr not allowed to move
// Can only be mutated locally by user or receive
#[derive(Debug)]
pub struct Mr {
    ptr: *mut [u8],
    mr: *const ibv_mr,
}

#[derive(Debug, Copy, Clone)]
pub struct RemoteMr {
    endpoint: (),
}

#[derive(Debug)]
pub struct RemoteMrSlice<'a> {
    mr: &'a RemoteMr,
    range: std::ops::Range<usize>,
}

impl RemoteMr {
    pub fn slice(&self, range: impl RangeBounds<usize>) -> RemoteMrSlice {
        RemoteMrSlice {
            mr: self,
            range: todo!(),
        }
    }
}

type ibv_mr = u8;

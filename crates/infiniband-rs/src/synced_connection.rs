use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use crate::connection::{CachedCompletionQueue, Connection, RemoteMr, RemoteMrSlice, WorkRequest};

pub struct SyncedIbConnection {
    inner: Connection,
}

impl SyncedIbConnection {
    pub fn register_mr(&mut self, name: impl Into<String>, region: *mut [u8]) -> io::Result<()> {
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
    pub unsafe fn share_mr(&mut self, name: impl AsRef<str>) -> io::Result<()> {
        //self.inner.share_mr(mr)
        todo!()
    }

    pub fn accept_shared_mr(&mut self) -> io::Result<RemoteMr> {
        //self.inner.accept_shared_mr()
        todo!()
    }

    pub fn remote_mr(&mut self, name: impl AsRef<str>) -> Option<RemoteMr> {
        //self.inner.remote_mr(name)
        todo!()
    }

    pub fn deregister_mr(&mut self, name: impl AsRef<str>) -> io::Result<()> {
        //self.inner.deregister_mr(mr)
        todo!()
    }

    // Scoping solves the problem of users being able to access memory regions scheduled for
    // an RDMA operation before it is complete. If the methods to send, receive, read, write, etc,
    // were in this class, the returned work requests could be dropped before the operation finished.
    // If the work requests implemented a Drop trait to poll before being dropped, the user could
    // forget them beforehand safely anyway, and so access the memory before the operation finished.
    // The solution for this, as proposed by Jonatan, is to use the same scoping method as the one used
    // for scoped treads. In this way, the created work requests have a well defined lifetime —that of
    // the scope— and are stored in a private structure such that the user cannot forget them to avoid polling.
    // If they have not been polled at the end of the scope, they will be polled automatically.
    pub fn scope<R>(&mut self, f: impl FnOnce(&mut ScopedSyncIbConnection) -> R) -> R {
        todo!()
    }
}

pub struct ScopedSyncIbConnection<'scope> {
    inner: &'scope mut SyncedIbConnection,
    wrs: Vec<WorkRequest<'scope>>,
    cq: Rc<RefCell<CachedCompletionQueue>>,
}

impl<'scope> ScopedSyncIbConnection<'scope> {
    // The slice cannot be used again until the work request is consumed,
    // so no overlapping sends can be done concurrently
    pub fn post_send<'a>(&mut self, slice: &'a [u8]) -> io::Result<WorkRequest<'a>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr)
    }

    // The slice cannot be used again until the work request is consumed,
    // so no overlapping receives can be done concurrently
    pub fn post_receive<'a>(&mut self, slice: &'a mut [u8]) -> io::Result<WorkRequest<'a>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr)
    }

    // Safety: The data at the remote memory region might be modified while the read is done.
    // It is the user's responsibility to ensure it is stable while the read is in progress.
    pub unsafe fn post_read<'a>(
        &mut self,
        from_slice: &RemoteMrSlice,
        into_slice: &'a mut [u8],
    ) -> io::Result<WorkRequest<'a>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr)
    }

    // Safety: The data at the remote memory region will be modified regardless of its mutability
    // status. It is the user's responsibility to ensure no use of the memory is being done concurrently.
    pub unsafe fn post_write<'a>(
        &mut self,
        from_slice: &'a [u8],
        into_slice: &RemoteMrSlice,
    ) -> io::Result<WorkRequest<'a>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr)
    }
}

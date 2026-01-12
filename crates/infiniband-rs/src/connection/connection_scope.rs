use crate::connection::connection::{IbvConnReceive, IbvConnSend, IbvConnection};
use crate::connection::work_request::{IbvWorkRequest, IbvWorkRequestStatus, IbvWorkResult};
use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;

pub struct IbvConnectionScope<'scope, 'env: 'scope> {
    inner: &'env mut IbvConnection,
    wrs: Vec<Rc<RefCell<IbvWorkRequest<'scope>>>>,
    // for invariance of lifetimes, see `std::thread::scope`
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

impl<'scope, 'env> Drop for IbvConnectionScope<'scope, 'env> {
    fn drop(&mut self) {
        self.wrs.iter().for_each(|wr| {
            let mut wr = wr.borrow_mut();
            if !wr.already_polled_to_completion() {
                log::warn!("IbvScopedWorkRequest not manually polled to completion");
                match wr.spin_poll() {
                    Ok(Err(op_error)) => log::error!(
                        "IbvScopedWorkRequest operation error detected on \
                             the scope's clean up: {op_error}"
                    ),
                    Err(io_error) => log::error!(
                        "Unable to poll IbvScopedWorkRequest to completion in \
                             the scope's clean up: {io_error}"
                    ),
                    Ok(Ok(_wc)) => {}
                }
            }
        });
    }
}

impl<'scope, 'env> IbvConnectionScope<'scope, 'env> {
    pub(super) fn new(connection: &'env mut IbvConnection) -> Self {
        IbvConnectionScope {
            inner: connection,
            wrs: vec![],
            scope: PhantomData,
            env: PhantomData,
        }
    }
}

impl<'scope, 'env> IbvConnectionScope<'scope, 'env> {
    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub fn post_send(
        &mut self,
        sends: impl AsRef<[IbvConnSend<'env>]>,
    ) -> io::Result<IbvScopedWorkRequest<'scope>> {
        let wr = Rc::new(RefCell::new(unsafe { self.inner.send_unpolled(sends)? }));
        self.wrs.push(wr.clone());
        Ok(IbvScopedWorkRequest {
            inner: wr,
            env: Default::default(),
        })
    }

    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub fn post_receive(
        &mut self,
        receives: impl AsRef<[IbvConnReceive<'env>]>,
    ) -> io::Result<IbvScopedWorkRequest<'scope>> {
        let wr = Rc::new(RefCell::new(unsafe {
            self.inner.receive_unpolled(receives)?
        }));
        self.wrs.push(wr.clone());
        Ok(IbvScopedWorkRequest {
            inner: wr,
            env: Default::default(),
        })
    }

    /*
    // Safety: The data at the remote memory region might be modified while the read is done.
    // It is the user's responsibility to ensure it is stable while the read is in progress.
    pub unsafe fn post_read(
        &'scope mut self,
        from_slice: &'env RemoteMrSlice,
        into_slice: &'env mut [u8],
    ) -> Result<IbvScopedWorkRequest<'scope, 'env>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr.into())
    }

    // Safety: The data at the remote memory region will be modified regardless of its mutability
    // status. It is the user's responsibility to ensure no use of the memory is being done concurrently.
    pub unsafe fn post_write(
        &'scope mut self,
        from_slice: &'env [u8],
        into_slice: &'env RemoteMrSlice,
    ) -> Result<IbvScopedWorkRequest<'scope, 'env>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr.into())
    }
    */
}

pub struct IbvScopedWorkRequest<'scope> {
    inner: Rc<RefCell<IbvWorkRequest<'scope>>>,
    env: PhantomData<&'scope mut &'scope ()>,
}

impl<'scope> IbvScopedWorkRequest<'scope> {
    pub fn poll(&self) -> io::Result<IbvWorkRequestStatus> {
        self.inner.borrow_mut().poll()
    }

    pub fn spin_poll(&self) -> io::Result<IbvWorkResult> {
        self.inner.borrow_mut().spin_poll()
    }
}

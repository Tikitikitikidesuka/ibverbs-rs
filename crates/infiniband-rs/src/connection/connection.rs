use crate::connection::cached_completion_queue::IbvCachedCompletionQueue;
use crate::connection::unsafe_member::UnsafeMember;
use crate::connection::work_completion::IbvWorkCompletion;
use crate::connection::work_error::IbvWorkError;
use crate::connection::work_request::IbvWorkRequest;
use crate::ibverbs::memory_region::IbvMemoryRegion;
use crate::ibverbs::protection_domain::IbvProtectionDomain;
use crate::ibverbs::queue_pair::IbvQueuePair;
use crate::ibverbs::queue_pair_builder::AccessFlags;
use ibverbs_sys::ibv_sge;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::marker::PhantomData;
use std::ops::Bound::{Excluded, Included};
use std::ops::RangeBounds;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug)]
pub struct IbvConnection {
    cq: Rc<RefCell<IbvCachedCompletionQueue>>,
    pd: IbvProtectionDomain,
    qp: IbvQueuePair,
    mrs: HashMap<String, IbvMemoryRegion>,
    next_wr_id: u64,
    //remote_mrs: HashMap<String, RemoteMr>,
}

impl IbvConnection {
    pub(super) fn new(
        cq: IbvCachedCompletionQueue,
        pd: IbvProtectionDomain,
        qp: IbvQueuePair,
    ) -> Self {
        Self {
            cq: Rc::new(RefCell::new(cq)),
            pd,
            qp,
            mrs: HashMap::new(),
            next_wr_id: 0,
        }
    }
}

#[derive(Debug, Error)]
pub enum IbvConnPolledWrError {
    #[error("io error: {0}")]
    IoError(#[from] io::Error),
    #[error("operation error: {0}")]
    OpError(#[from] IbvWorkError),
}

impl IbvConnection {
    pub fn register_mr(
        &mut self,
        name: impl Into<String>,
        address: *mut u8,
        length: usize,
    ) -> io::Result<IbvConnMr> {
        let name = name.into();
        if self.mrs.contains_key(&name) {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("memory region \"{name}\" already registered"),
            ));
        }

        let mr = unsafe {
            self.pd.register_mr_with_permissions(
                address,
                length,
                // TODO: Start with only local_write and add remote_read and remote_write when shared
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write()
                    .into(),
            )?
        };

        let out_mr = IbvConnMr {
            lkey: mr.lkey(),
            address: mr.address(),
            length: mr.length(),
        };

        self.mrs.insert(name, mr);

        Ok(out_mr)
    }

    pub fn register_dmabuf_mr(
        &mut self,
        name: impl Into<String>,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<IbvConnMr> {
        let name = name.into();
        if self.mrs.contains_key(&name) {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("memory region \"{name}\" already registered"),
            ));
        }

        let mr = unsafe {
            self.pd.register_dmabuf(
                fd,
                offset,
                length,
                iova,
                // TODO: Start with only local_write and add remote_read and remote_write when shared
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write()
                    .into(),
            )?
        };

        let out_mr = IbvConnMr {
            lkey: mr.lkey(),
            address: mr.address(),
            length: mr.length(),
        };

        self.mrs.insert(name, mr);

        Ok(out_mr)
    }

    pub fn get_mr(&self, name: impl AsRef<str>) -> Option<IbvConnMr> {
        self.mrs.get(name.as_ref()).map(|mr| IbvConnMr {
            lkey: mr.lkey(),
            address: mr.address(),
            length: mr.length(),
        })
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
        let name = name.as_ref();
        if let None = self.mrs.remove(name) {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("memory region \"{name}\" not registered"),
            ))
        } else {
            Ok(())
        }
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

    // todo do those actually need mutable access? -> Yes for non atomic wr id counter

    // todo do we want to return the poll duration / number of local bytes written? -> Work completions with all the info
    // todo do these functions assert that the slice length maches exact? how would we do that?
    // todo what about immediate data? extra function or include?
    pub fn send<'a>(
        &mut self,
        sends: impl AsRef<[IbvConnSend<'a>]>,
    ) -> Result<IbvWorkCompletion, IbvConnPolledWrError> {
        Ok(unsafe { self.send_unpolled(sends)? }.consume()??)
    }

    pub fn send_with_imm_data<'a>(
        &mut self,
        sends: impl AsRef<[IbvConnSend<'a>]>,
        imm_data: u32,
    ) -> Result<IbvWorkCompletion, IbvConnPolledWrError> {
        Ok(unsafe { self.send_with_imm_data_unpolled(sends, imm_data)? }.consume()??)
    }

    pub fn receive<'a>(
        &mut self,
        receives: impl AsRef<[IbvConnReceive<'a>]>,
    ) -> Result<IbvWorkCompletion, IbvConnPolledWrError> {
        Ok(unsafe { self.receive_unpolled(receives)? }.consume()??)
    }

    // unsafe functions

    /// # Safety
    /// The caller must ensure that the work request is successfully polled to completion before the end of `'a`.
    pub unsafe fn send_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[IbvConnSend<'a>]>,
    ) -> io::Result<IbvWorkRequest<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_send(sends.as_ref().as_sge_slice(), wr_id)? };
        Ok(unsafe { IbvWorkRequest::new(wr_id, self.cq.clone()) })
    }

    /// # Safety
    /// The caller must ensure that the work request is successfully polled to completion before the end of `'a`.
    pub unsafe fn send_with_imm_data_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[IbvConnSend<'a>]>,
        imm_data: u32,
    ) -> io::Result<IbvWorkRequest<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe {
            self.qp
                .post_send_with_imm(sends.as_ref().as_sge_slice(), imm_data, wr_id)?
        };
        Ok(unsafe { IbvWorkRequest::new(wr_id, self.cq.clone()) })
    }

    /// # Safety
    /// The caller must ensure that the work request is successfully polled to completion before the end of `'a`.
    pub unsafe fn receive_unpolled<'a>(
        &mut self,
        receives: impl AsRef<[IbvConnReceive<'a>]>,
    ) -> io::Result<IbvWorkRequest<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe {
            self.qp
                .post_receive(receives.as_ref().as_sge_slice(), wr_id)?
        };
        Ok(unsafe { IbvWorkRequest::new(wr_id, self.cq.clone()) })
    }

    /// # Safety
    /// This method is unsafe because ...
    /// todo, do we need to make it unsafe if it does unsafe things on the *other* side?
    ///
    /// Furthermore, he caller must ensure that the work request is sucessfully polled to completion before the end of `'a`.
    pub unsafe fn remote_write<'a>(
        &mut self,
        data: &'a [u8],
        remote_slice: RemoteMrSlice,
    ) -> io::Result<IbvWorkRequest<'a>> {
        todo!()
    }

    /// # Safety
    /// This method is unsafe because ...
    /// todo
    ///
    /// Furthermore, the caller must ensure that the work request is sucessfully polled to completion before the end of `'a`.
    pub unsafe fn remote_read<'a>(
        &mut self,
        remote_slice: RemoteMrSlice,
        data: &'a mut [u8],
    ) -> io::Result<IbvWorkRequest<'a>> {
        todo!()
    }

    fn get_and_advance_wr_id(&mut self) -> u64 {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;
        wr_id
    }
}

/*
pub struct IbvConnectionScope<'scope, 'env: 'scope> {
    inner: &'scope mut IbvConnection,
    wrs: Vec<Rc<RefCell<IbvWorkRequest<'scope>>>>,
    // for invariance of lifetimes, see `std::thread::scope`
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

pub struct IbvScopedWorkRequest<'scope, 'env: 'scope> {
    inner: IbvWorkRequest<'env>,
    env: PhantomData<&'scope mut &'scope ()>,
}

impl<'scope, 'env> From<IbvWorkRequest<'env>> for IbvScopedWorkRequest<'scope, 'env> {
    fn from(value: IbvWorkRequest<'env>) -> Self {
        IbvScopedWorkRequest {
            inner: value,
            env: PhantomData,
        }
    }
}

impl<'scope, 'env> IbvConnectionScope<'scope, 'env> {
    // The slice cannot be used again until the work request is consumed,
    // so no overlapping sends can be done concurrently
    pub fn post_send(
        &'scope mut self,
        slice: &'env [u8],
    ) -> io::Result<IbvScopedWorkRequest<'scope, 'env>> {
        // TODO: Post to infiniband hardware

        let wr = Rc::new(RefCell::new(unsafe {
            IbvWorkRequest::new(self.inner.get_and_advance_wr_id(), self.inner.cq.clone())
        }));

        self.wrs.push(wr.clone());

        Ok(wr.into())
    }

    // The slice cannot be used again until the work request is consumed,
    // so no overlapping receives can be done concurrently
    pub fn post_receive(
        &'scope mut self,
        slice: &'env mut [u8],
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
}
 */

// Safety: memory of an mr not allowed to move
// Can only be mutated locally by user or receive
#[derive(Debug, Copy, Clone)]
pub struct IbvConnMr {
    lkey: u32,
    address: *const u8,
    length: usize,
}

#[derive(Debug, Error)]
pub enum IbvConnMrSliceError {
    #[error("maximum length of mr slice exceeded")]
    SliceTooBig,
    #[error("slice is not within the bounds of the mr")]
    SliceNotWithinBounds,
}

impl IbvConnMr {
    pub fn prepare_send<'a>(&self, data: &'a [u8]) -> Result<IbvConnSend<'a>, IbvConnMrSliceError> {
        let data_length = data
            .len()
            .try_into()
            .map_err(|error| IbvConnMrSliceError::SliceTooBig)?;
        if !self.data_is_contained(data.as_ptr(), data.len()) {
            return Err(IbvConnMrSliceError::SliceNotWithinBounds);
        }

        Ok(IbvConnSend {
            sge: ibv_sge {
                addr: data.as_ptr() as u64,
                length: data_length,
                lkey: self.lkey,
            },
            _data_lifetime: unsafe { UnsafeMember::new(Default::default()) },
        })
    }

    pub fn prepare_receive<'a>(
        &self,
        data: &'a mut [u8],
    ) -> Result<IbvConnReceive<'a>, IbvConnMrSliceError> {
        let data_length = data
            .len()
            .try_into()
            .map_err(|error| IbvConnMrSliceError::SliceTooBig)?;
        if !self.data_is_contained(data.as_ptr(), data.len()) {
            return Err(IbvConnMrSliceError::SliceNotWithinBounds);
        }

        Ok(IbvConnReceive {
            sge: ibv_sge {
                addr: data.as_ptr() as u64,
                length: data_length,
                lkey: self.lkey,
            },
            _data_lifetime: unsafe { UnsafeMember::new(Default::default()) },
        })
    }

    fn data_is_contained(&self, data_address: *const u8, data_length: usize) -> bool {
        let mr_start = self.address as usize;
        let mr_end = mr_start + self.length;
        let data_start = data_address as usize;
        let data_end = data_start + data_length;
        data_start >= mr_start && data_end <= mr_end
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct IbvConnSend<'a> {
    sge: ibv_sge,
    /// SAFETY INVARIANT: The lifetime of the data must be the same as the lifetime of the send.
    _data_lifetime: UnsafeMember<PhantomData<&'a [u8]>>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct IbvConnReceive<'a> {
    sge: ibv_sge,
    /// SAFETY INVARIANT: The lifetime of the data must be the same as the lifetime of the receive.
    _data_lifetime: UnsafeMember<PhantomData<&'a mut [u8]>>,
}

pub trait AsSgeSlice {
    fn as_sge_slice(&self) -> &[ibv_sge];
}

impl<'a> AsSgeSlice for [IbvConnSend<'a>] {
    fn as_sge_slice(&self) -> &[ibv_sge] {
        // Safe because `IbvConnSend<'a>` is `#[repr(transparent)]` to `ibv_sge`
        unsafe { std::slice::from_raw_parts(self.as_ptr() as *const ibv_sge, self.len()) }
    }
}

impl<'a> AsSgeSlice for [IbvConnReceive<'a>] {
    fn as_sge_slice(&self) -> &[ibv_sge] {
        // Safe because `IbvConnSend<'a>` is `#[repr(transparent)]` to `ibv_sge`
        unsafe { std::slice::from_raw_parts(self.as_ptr() as *const ibv_sge, self.len()) }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RemoteMr {
    endpoint: (),
}

// todo why take a reference if `RemoteMr' is `Copy`?
#[derive(Debug)]
pub struct RemoteMrSlice {
    mr: RemoteMr,
    range: std::ops::Range<usize>,
}

impl RemoteMr {
    pub fn slice(&self, range: impl RangeBounds<usize>) -> RemoteMrSlice {
        RemoteMrSlice {
            mr: *self,
            range: match (range.start_bound().cloned(), range.end_bound().cloned()) {
                (Included(a), Included(b)) => a..b + 1,
                (Included(a), Excluded(b)) => a..b,
                (Included(_), std::ops::Bound::Unbounded) => todo!(),
                (Excluded(a), Included(b)) => a + 1..b + 1,
                (Excluded(a), Excluded(b)) => a + 1..b,
                (Excluded(_), std::ops::Bound::Unbounded) => todo!(),
                (std::ops::Bound::Unbounded, Included(_)) => todo!(),
                (std::ops::Bound::Unbounded, Excluded(_)) => todo!(),
                (std::ops::Bound::Unbounded, std::ops::Bound::Unbounded) => todo!(),
            },
        }
    }
}

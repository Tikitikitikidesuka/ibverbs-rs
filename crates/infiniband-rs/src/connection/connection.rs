use crate::connection::cached_completion_queue::IbvCachedCompletionQueue;
use crate::connection::connection_scope::{IbvConnectionScope, IbvConnectionScopeError};
use crate::connection::unsafe_member::UnsafeMember;
use crate::connection::work_request::{IbvWorkRequest, IbvWorkSpinPollResult};
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
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug)]
// Order of attributes matters.
// Deallocation must happen in the order specified.
pub struct IbvConnection {
    qp: IbvQueuePair,
    mrs: HashMap<String, IbvMemoryRegion>,
    //remote_mrs: HashMap<String, RemoteMr>,
    cq: Rc<RefCell<IbvCachedCompletionQueue>>,
    pd: IbvProtectionDomain,
    next_wr_id: u64,
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

impl IbvConnection {
    pub fn register_mr(
        &mut self,
        name: impl Into<String>,
        memory: &mut [u8],
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
                memory.as_mut_ptr(),
                memory.len(),
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
    ///
    /// # Lifetimes
    ///
    /// Scoped rdma involves two lifetimes: `'scope` and `'env`.
    ///
    /// The `'scope` lifetime represents the lifetime of the scope itself.
    /// That is: the time during which new rdma operations may be issued,
    /// and also the time during which they might still be running.
    /// Once this lifetime ends, all operations are polled to completion.
    /// This lifetime starts within the `scope` function, before `f` (the argument to `scope`) starts.
    /// It ends after `f` returns and all scoped rdma operations have been completed, but before `scope` returns.
    ///
    /// The `'env` lifetime represents the lifetime of whatever is borrowed by the scoped threads.
    /// This lifetime must outlast the call to `scope`, and thus cannot be smaller than `'scope`.
    /// It can be as small as the call to `scope`, meaning that anything that outlives this call,
    /// such as local variables defined right before the scope, can be borrowed by the scope.
    ///
    /// The `'env: 'scope` bound is part of the definition of the `IbvConnectionScope` type.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    ///
    /// let (send_mem, recv_mem) = mem.split_at_mut(4);
    /// send_mem.copy_from_slice(&[1, 2, 3, 4]);
    /// conn.scope(|s| {
    ///     let wr0 = s.post_receive(&[mr.prepare_receive(recv_mem).unwrap()])
    ///     .unwrap();
    ///     let wr1 = s.post_send(&[mr.prepare_send(send_mem).unwrap()]).unwrap();
    ///     std::mem::forget(wr0);
    ///     std::mem::forget(wr1);
    /// });
    /// ```
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, IbvConnectionScopeError>
    where
        F: for<'scope> FnOnce(&mut IbvConnectionScope<'scope, 'env>) -> R,
    {
        let mut scope = IbvConnectionScope::new(self);
        // The user's closure may panic after issuing work requests.
        // The panic has to be caught to ensure clean up for exception safety.
        let user_result = catch_unwind(AssertUnwindSafe(|| f(&mut scope)));
        let clean_up_result = scope.clean_up();
        match user_result {
            Ok(r) => clean_up_result.map(|_| r),
            Err(panic) => resume_unwind(panic),
        }
    }

    pub fn send<'a>(&mut self, sends: impl AsRef<[IbvConnSend<'a>]>) -> IbvWorkSpinPollResult {
        Ok(unsafe { self.send_unpolled(sends)? }.spin_poll()?)
    }

    pub fn send_with_imm_data<'a>(
        &mut self,
        sends: impl AsRef<[IbvConnSend<'a>]>,
        imm_data: u32,
    ) -> IbvWorkSpinPollResult {
        Ok(unsafe { self.send_with_imm_data_unpolled(sends, imm_data)? }.spin_poll()?)
    }

    pub fn receive<'a>(
        &mut self,
        receives: impl AsRef<[IbvConnReceive<'a>]>,
    ) -> IbvWorkSpinPollResult {
        Ok(unsafe { self.receive_unpolled(receives)? }.spin_poll()?)
    }

    // unsafe functions

    /// # Safety
    /// The caller must ensure that the returned `IbvWorkRequest` is
    /// **successfully polled to completion by its drop implementation**
    /// before the end of `'a`.
    ///
    /// In particular, the work request must not be leaked (e.g. via
    /// `mem::forget`), as this would end the borrow without dropping
    /// while the hardware may still access the memory.
    ///
    /// ## Protection example
    ///
    /// ```compile_fail
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_send(&mem[0..4]).unwrap();
    /// let wr = unsafe { conn.send_unpolled(&[receive]) }.unwrap();
    ///
    /// // This mutation of mem will not compile while the borrow is alive in the wr,
    /// // preventing partially modified memory from being sent.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
    ///
    /// ## Safety violation example
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.receive_unpolled(&[receive]) }.unwrap();
    ///
    /// // The work request can be leaked without running its drop.
    /// // The borrow ends but the NIC may still DMA into the memory.
    /// std::mem::forget(wr);
    ///
    /// // This mutation of mem might occur while the send is partially complete.
    /// // This violates Rust's aliasing rules and constitutes UB.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
    pub unsafe fn send_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[IbvConnSend<'a>]>,
    ) -> io::Result<IbvWorkRequest<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_send(sends.as_ref().as_sge_slice(), wr_id)? };
        Ok(unsafe { IbvWorkRequest::new(wr_id, self.cq.clone()) })
    }

    /// # Safety
    ///
    /// The caller must ensure that the returned `IbvWorkRequest` is
    /// **successfully polled to completion by its drop implementation**
    /// before the end of `'a`.
    ///
    /// In particular, the work request must not be leaked (e.g. via
    /// `mem::forget`), as this would end the borrow without dropping
    /// while the hardware may still access the memory.
    ///
    /// ## Protection example
    ///
    /// ```compile_fail
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_send(&mem[0..4]).unwrap();
    /// let wr = unsafe { conn.send_with_imm_data_unpolled(&[receive], 33) }.unwrap();
    ///
    /// // This mutation of mem will not compile while the borrow is alive in the wr,
    /// // preventing partially modified memory from being sent.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
    ///
    /// ## Safety violation example
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.send_with_imm_data_unpolled(&[receive], 33) }.unwrap();
    ///
    /// // The work request can be leaked without running its drop.
    /// // The borrow ends but the NIC may still DMA into the memory.
    /// std::mem::forget(wr);
    ///
    /// // This mutation of mem might occur while the send is partially complete.
    /// // This violates Rust's aliasing rules and constitutes UB.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
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
    /// The caller must ensure that the returned `IbvWorkRequest` is
    /// **successfully polled to completion by its drop implementation**
    /// before the end of `'a`.
    ///
    /// In particular, the work request must not be leaked (e.g. via
    /// `mem::forget`), as this would end the borrow without dropping
    /// while the hardware may still access the memory.
    ///
    /// ## Protection example
    ///
    /// ```compile_fail
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.receive_unpolled(&[receive]) }.unwrap();
    ///
    /// // This read of mem will not compile while the borrow is alive in the wr.
    /// println!("{:?}", &mem[0..4]);
    /// ```
    ///
    /// ## Safety violation example
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.receive_unpolled(&[receive]) }.unwrap();
    ///
    /// // The work request can be leaked without running its drop.
    /// // The borrow ends but the NIC may still DMA into the memory.
    /// std::mem::forget(wr);
    ///
    /// // This read of mem might occur while the receive is partially complete.
    /// // This violates Rust's aliasing rules and constitutes UB.
    /// println!("{:?}", &mem[0..4]);
    /// ```
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
    /// This method is unsafe because ... (same reason as send) memory being written must respect &[] aliasing
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
    /// This method is unsafe because ... (same reason as receive) memory being read into must respect &mut[] aliasing
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
            .map_err(|_| IbvConnMrSliceError::SliceTooBig)?;
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

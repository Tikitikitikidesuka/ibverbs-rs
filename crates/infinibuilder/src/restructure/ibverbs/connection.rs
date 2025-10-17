use crate::restructure::ibverbs::completion_queue::CachedCompletionQueue;
use crate::restructure::ibverbs::memory_region::{IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::restructure::ibverbs::work_completion::IbvWorkCompletion;
use crate::restructure::ibverbs::work_request::IbvWorkRequest;
use crate::restructure::rdma_connection::RdmaConnection;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, Context, PreparedQueuePair, ProtectionDomain, QueuePair, QueuePairEndpoint,
    RemoteMemoryRegion,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::RangeBounds;
use std::rc::Rc;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbvConnectionBuilderError {
    #[error("Device list is unaccessible: {0}")]
    DeviceListUnaccessible(std::io::Error),
    #[error("Device is unaccessible: {0}")]
    DeviceUnaccessible(std::io::Error),
    #[error("Device with name \"{0}\" not found")]
    DeviceNameNotFound(String),
    #[error("Unable to create completion queue")]
    CompletionQueueCreationError(std::io::Error),
    #[error("Unable to create protection domain")]
    ProtectionDomainCreationError(std::io::Error),
    #[error("Unable to create queue pair")]
    QueuePairCreationError(std::io::Error),
    #[error("Unable to register memory region")]
    MemoryRegionRegisterError(std::io::Error),
    #[error("Memory region with id \"{0}\" already registered")]
    MemoryRegionDuplicateRegister(String),
    #[error("Unable to connect: {0}")]
    ConnectionError(std::io::Error),
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvConnectionBuilder<CTX, QP, PD, CQ> {
    ctx: CTX,
    qp: QP,
    #[derivative(Debug = "ignore")]
    pd: PD,
    #[derivative(Debug = "ignore")]
    cq: CQ,
    mr_endpoints: HashMap<String, IbvRemoteMemoryRegion>,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvPreparedConnection {
    local_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    qp: PreparedQueuePair,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    mr_endpoints: Vec<(String, IbvRemoteMemoryRegion)>,
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct BuilderContext {
    device_name: String,
    #[derivative(Debug = "ignore")]
    context: Arc<Context>, // Arc to make it cloneable
}
#[derive(Derivative)]
#[derivative(Debug)]
pub struct BuilderQueuePair {
    qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    qp: PreparedQueuePair,
}
#[derive(Derivative)]
#[derivative(Debug)]
pub struct BuilderCompletionQueue {
    capacity: i32,
    cache_capacity: usize,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

// Allow cloning when context is initialized but the rest is not to avoid opening it multiple times
impl Clone for IbvConnectionBuilder<BuilderContext, (), (), ()> {
    fn clone(&self) -> Self {
        Self {
            ctx: self.ctx.clone(),
            qp: (),
            pd: (),
            cq: (),
            mr_endpoints: HashMap::new(),
        }
    }
}

impl IbvConnectionBuilder<(), (), (), ()> {
    pub fn new() -> Self {
        Self {
            ctx: (),
            qp: (),
            pd: (),
            cq: (),
            mr_endpoints: HashMap::new(),
        }
    }

    pub fn with_ibv_device(
        self,
        device_name: impl Into<String>,
    ) -> Result<
        IbvConnectionBuilder<BuilderContext, (), (), ()>,
        IbvConnectionBuilderError,
    > {
        let device_name = device_name.into();
        let context = ibverbs::devices()
            .map_err(|e| IbvConnectionBuilderError::DeviceListUnaccessible(e))?
            .iter()
            .find(|d| match d.name() {
                None => false,
                Some(name) => name.to_string_lossy() == device_name,
            })
            .ok_or(IbvConnectionBuilderError::DeviceNameNotFound(
                device_name.clone(),
            ))?
            .open()
            .map_err(|e| IbvConnectionBuilderError::DeviceUnaccessible(e))?;

        Ok(IbvConnectionBuilder {
            ctx: BuilderContext {
                context: Arc::new(context),
                device_name,
            },
            qp: self.qp,
            pd: self.pd,
            cq: self.cq,
            mr_endpoints: self.mr_endpoints,
        })
    }
}

impl<PD> IbvConnectionBuilder<BuilderContext, (), PD, ()> {
    pub fn create_cq(
        self,
        capacity: i32,
        cache_capacity: usize,
    ) -> Result<
        IbvConnectionBuilder<BuilderContext, (), PD, BuilderCompletionQueue>,
        IbvConnectionBuilderError,
    > {
        let cq = self
            .ctx
            .context
            .create_cq(capacity, 0)
            .map_err(|e| IbvConnectionBuilderError::CompletionQueueCreationError(e))?;

        Ok(IbvConnectionBuilder {
            ctx: self.ctx,
            qp: self.qp,
            pd: self.pd,
            cq: BuilderCompletionQueue {
                capacity,
                cache_capacity,
                cq,
            },
            mr_endpoints: self.mr_endpoints,
        })
    }
}

impl<CQ> IbvConnectionBuilder<BuilderContext, (), (), CQ> {
    pub fn create_pd(
        self,
    ) -> Result<
        IbvConnectionBuilder<BuilderContext, (), ProtectionDomain, CQ>,
        IbvConnectionBuilderError,
    > {
        let pd = self
            .ctx
            .context
            .alloc_pd()
            .map_err(|e| IbvConnectionBuilderError::ProtectionDomainCreationError(e))?;

        Ok(IbvConnectionBuilder {
            ctx: self.ctx,
            qp: self.qp,
            pd,
            cq: self.cq,
            mr_endpoints: self.mr_endpoints,
        })
    }
}

impl<QP, CQ> IbvConnectionBuilder<BuilderContext, QP, ProtectionDomain, CQ> {
    pub fn register_mr(
        &mut self,
        id: impl Into<String>,
        ptr: *mut u8,
        length: usize,
    ) -> Result<IbvMemoryRegion, IbvConnectionBuilderError> {
        let id = id.into();
        if self.mr_endpoints.contains_key(&id) {
            // If mr with same id is already registered, fail
            Err(IbvConnectionBuilderError::MemoryRegionDuplicateRegister(id))
        } else {
            // Otherwise, register memory
            let mr = self
                .pd
                .register(ptr, length)
                .map_err(|e| IbvConnectionBuilderError::MemoryRegionRegisterError(e))?;

            // And keep track of it
            self.mr_endpoints.insert(
                id,
                IbvRemoteMemoryRegion {
                    length,
                    rmr: mr.remote(),
                },
            );

            Ok(IbvMemoryRegion { length, mr })
        }
    }
}

impl IbvConnectionBuilder<BuilderContext, (), ProtectionDomain, BuilderCompletionQueue> {
    pub fn create_qp(
        self,
    ) -> Result<
        IbvConnectionBuilder<
            BuilderContext,
            BuilderQueuePair,
            ProtectionDomain,
            BuilderCompletionQueue,
        >,
        IbvConnectionBuilderError,
    > {
        let qp = self
            .pd
            .create_qp(&self.cq.cq, &self.cq.cq, ibverbs::ibv_qp_type::IBV_QPT_RC)
            .map_err(|e| IbvConnectionBuilderError::QueuePairCreationError(e))?
            .set_access(
                ibverbs::ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibverbs::ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    | ibverbs::ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
            )
            .build()
            .map_err(|e| IbvConnectionBuilderError::QueuePairCreationError(e))?;
        let qp_endpoint = qp
            .endpoint()
            .map_err(|e| IbvConnectionBuilderError::QueuePairCreationError(e))?;

        Ok(IbvConnectionBuilder {
            ctx: self.ctx,
            qp: BuilderQueuePair { qp, qp_endpoint },
            pd: self.pd,
            cq: self.cq,
            mr_endpoints: self.mr_endpoints,
        })
    }
}

impl
    IbvConnectionBuilder<BuilderContext, BuilderQueuePair, ProtectionDomain, BuilderCompletionQueue>
{
    pub fn build(self) -> IbvPreparedConnection {
        IbvPreparedConnection {
            local_qp_endpoint: self.qp.qp_endpoint,
            qp: self.qp.qp,
            pd: self.pd,
            cq: Rc::new(RefCell::new(CachedCompletionQueue::new(
                self.cq.cq,
                self.cq.cache_capacity,
            ))),
            mr_endpoints: self.mr_endpoints.into_iter().map(|r| r.into()).collect(),
        }
    }
}

impl IbvPreparedConnection {
    pub fn endpoint(&self) -> IbvConnectionEndpoint {
        IbvConnectionEndpoint {
            qp_endpoint: self.local_qp_endpoint,
            mr_endpoints: self.mr_endpoints.clone(),
        }
    }

    pub fn connect(
        self,
        connection_config: IbvConnectionEndpoint,
    ) -> Result<IbvConnection, IbvConnectionBuilderError> {
        let qp = self
            .qp
            .handshake(connection_config.qp_endpoint)
            .map_err(|e| IbvConnectionBuilderError::ConnectionError(e))?;

        let mr_endpoints = connection_config.mr_endpoints.into_iter().collect();

        Ok(IbvConnection {
            local_qp_endpoint: self.local_qp_endpoint,
            remote_qp_endpoint: connection_config.qp_endpoint,
            qp,
            _pd: self.pd,
            cq: self.cq,
            mr_endpoints,
            next_wr_id: 0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbvConnectionEndpoint {
    qp_endpoint: QueuePairEndpoint,
    mr_endpoints: Vec<(String, IbvRemoteMemoryRegion)>,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvConnection {
    local_qp_endpoint: QueuePairEndpoint,
    remote_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    qp: QueuePair,
    #[derivative(Debug = "ignore")]
    _pd: ProtectionDomain,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    mr_endpoints: HashMap<String, IbvRemoteMemoryRegion>,
    next_wr_id: u64,
}

impl IbvConnection {
    fn next_wr_id(&mut self) -> u64 {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;
        wr_id
    }
}

impl IbvConnection {
    pub fn remote_mr(&self, id: impl AsRef<str>) -> Option<IbvRemoteMemoryRegion> {
        self.mr_endpoints.get(id.as_ref()).cloned()
    }
}

impl RdmaConnection for IbvConnection {
    type MR = IbvMemoryRegion;
    type RemoteMR = IbvRemoteMemoryRegion;
    type WR = IbvWorkRequest;
    type WC = IbvWorkCompletion;
    type PostError = std::io::Error;

    fn post_send(
        &mut self,
        memory_region: Self::MR,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError> {
        let wr_id = self.next_wr_id();
        unsafe {
            self.qp.post_send(
                &[memory_region.mr.slice(memory_range)],
                wr_id,
                immediate_data,
            )
        }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }

    fn post_receive(
        &mut self,
        memory_region: Self::MR,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        let wr_id = self.next_wr_id();
        unsafe {
            self.qp
                .post_receive(&[memory_region.mr.slice(memory_range)], wr_id)
        }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }

    fn post_write(
        &mut self,
        local_memory_region: Self::MR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: Self::RemoteMR,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError> {
        let wr_id = self.next_wr_id();
        self.qp.post_write(
            &[local_memory_region.mr.slice(local_memory_range)],
            remote_memory_region.rmr.slice(remote_memory_range),
            wr_id,
            immediate_data,
        )?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }

    fn post_read(
        &mut self,
        local_memory_region: Self::MR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: Self::RemoteMR,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        let wr_id = self.next_wr_id();
        self.qp.post_read(
            &[local_memory_region.mr.slice(local_memory_range)],
            remote_memory_region.rmr.slice(remote_memory_range),
            wr_id,
        )?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }

    fn post_send_immediate_data(
        &mut self,
        immediate_data: u32,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id();
        unsafe { self.qp.post_send(&[], wr_id, Some(immediate_data)) }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }

    fn post_receive_immediate_data(&mut self) -> Result<IbvWorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id();
        unsafe { self.qp.post_receive(&[], wr_id) }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }
}

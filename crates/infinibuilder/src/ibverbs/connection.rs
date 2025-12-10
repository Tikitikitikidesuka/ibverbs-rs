use crate::ibverbs::completion_queue::CachedCompletionQueue;
use crate::ibverbs::work_request::IbvWorkRequest;
use crate::ibverbs::{self, Named};
use crate::rdma_connection::{
    RdmaMemoryRegionConnection, RdmaNamedMemoryRegionConnection,
    RdmaNamedRemoteMemoryRegionConnection, RdmaPostReadConnection, RdmaPostReceiveConnection,
    RdmaPostReceiveImmediateDataConnection, RdmaPostSendConnection,
    RdmaPostSendImmediateDataConnection, RdmaPostWriteConnection, RdmaRemoteMemoryRegionConnection,
};
use crate::rdma_network_node::RdmaNamedMemory;
use ::ibverbs::{
    CompletionQueue, Context, DEFAULT_ACCESS_FLAGS, MemoryRegion, PreparedQueuePair,
    ProtectionDomain, QueuePair, QueuePairEndpoint, RemoteMemoryRegion, ibv_access_flags,
};
use derivative::Derivative;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::rc::Rc;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbvConnectionBuildError {
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

#[derive(Debug)]
pub struct IbvConnectionBuilder<IbvDevName, CqParams, Mrs> {
    ibv_device_name: IbvDevName,
    cq_params: CqParams,
    mrs: Mrs,
}

#[derive(Debug, Clone)]
pub struct BuilderIbvDeviceName {
    ibv_device_name: String,
}

#[derive(Debug, Clone)]
pub struct BuilderCqParams {
    capacity: usize,
    cache_capacity: usize,
}

#[derive(Debug)]
pub struct BuilderMemoryRegions {
    mrs: Vec<RdmaNamedMemory>,
}

// Builder is cloneable until memory regions are specified
impl<IbvDevName: Clone, CqParams: Clone> Clone for IbvConnectionBuilder<IbvDevName, CqParams, ()> {
    fn clone(&self) -> Self {
        Self {
            ibv_device_name: self.ibv_device_name.clone(),
            cq_params: self.cq_params.clone(),
            mrs: self.mrs,
        }
    }
}

impl IbvConnectionBuilder<(), (), ()> {
    pub fn new() -> Self {
        Self {
            ibv_device_name: (),
            cq_params: (),
            mrs: (),
        }
    }
}

impl<CqParams, Mrs> IbvConnectionBuilder<(), CqParams, Mrs> {
    pub fn ibv_device(
        self,
        device_name: impl Into<String>,
    ) -> IbvConnectionBuilder<BuilderIbvDeviceName, CqParams, Mrs> {
        IbvConnectionBuilder {
            ibv_device_name: BuilderIbvDeviceName {
                ibv_device_name: device_name.into(),
            },
            cq_params: self.cq_params,
            mrs: self.mrs,
        }
    }
}

impl<IbvDevName, Mrs> IbvConnectionBuilder<IbvDevName, (), Mrs> {
    pub fn cq_params(
        self,
        capacity: usize,
        cache_capacity: usize,
    ) -> IbvConnectionBuilder<IbvDevName, BuilderCqParams, Mrs> {
        IbvConnectionBuilder {
            ibv_device_name: self.ibv_device_name,
            cq_params: BuilderCqParams {
                capacity,
                cache_capacity,
            },
            mrs: self.mrs,
        }
    }
}

impl IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, ()> {
    pub fn lock_clone(
        self,
    ) -> IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, BuilderMemoryRegions> {
        IbvConnectionBuilder {
            ibv_device_name: self.ibv_device_name,
            cq_params: self.cq_params,
            mrs: BuilderMemoryRegions { mrs: vec![] },
        }
    }
}

impl IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, BuilderMemoryRegions> {
    pub fn register_mr(
        self,
        memory: RdmaNamedMemory,
    ) -> IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, BuilderMemoryRegions> {
        let mut mrs = self.mrs;
        mrs.mrs.push(memory);

        IbvConnectionBuilder {
            ibv_device_name: self.ibv_device_name,
            cq_params: self.cq_params,
            mrs,
        }
    }

    pub fn register_mrs(
        mut self,
        mrs: impl IntoIterator<Item = RdmaNamedMemory>,
    ) -> IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, BuilderMemoryRegions> {
        self.mrs
            .mrs
            .append(&mut mrs.into_iter().collect::<Vec<_>>());

        IbvConnectionBuilder {
            ibv_device_name: self.ibv_device_name,
            cq_params: self.cq_params,
            mrs: self.mrs,
        }
    }
}

impl IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, ()> {
    pub fn build(self) -> Result<IbvPreparedConnection, IbvConnectionBuildError> {
        IbvConnectionBuilder {
            ibv_device_name: self.ibv_device_name,
            cq_params: self.cq_params,
            mrs: BuilderMemoryRegions { mrs: vec![] },
        }
        .build()
    }
}

impl IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, BuilderMemoryRegions> {
    pub fn build(self) -> Result<IbvPreparedConnection, IbvConnectionBuildError> {
        let context = self.open_context()?;
        let pd = self.create_pd(&context)?;
        let cq = self.create_cq(&context)?;
        let qp = self.create_qp(&pd, &cq)?;
        let local_qp_endpoint = self.qp_endpoint(&qp)?;
        let mrs = self.inner_register_mrs(&pd)?;

        let cq = Rc::new(RefCell::new(CachedCompletionQueue::new(
            cq,
            self.cq_params.cache_capacity,
        )));

        Ok(IbvPreparedConnection {
            local_qp_endpoint,
            qp,
            pd,
            cq,
            mrs,
        })
    }

    fn open_context(&self) -> Result<Context, IbvConnectionBuildError> {
        ::ibverbs::devices()
            .map_err(|e| IbvConnectionBuildError::DeviceListUnaccessible(e))?
            .iter()
            .find(|d| match d.name() {
                None => false,
                Some(name) => name.to_string_lossy() == self.ibv_device_name.ibv_device_name,
            })
            .ok_or(IbvConnectionBuildError::DeviceNameNotFound(
                self.ibv_device_name.ibv_device_name.clone(),
            ))?
            .open()
            .map_err(|e| IbvConnectionBuildError::DeviceUnaccessible(e))
    }

    fn create_cq(&self, context: &Context) -> Result<CompletionQueue, IbvConnectionBuildError> {
        let capacity = match i32::try_from(self.cq_params.capacity) {
            Ok(capacity) => capacity,
            Err(_) => {
                return Err(IbvConnectionBuildError::QueuePairCreationError(
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "asdf"),
                ));
            }
        };

        context
            .create_cq(capacity, 0)
            .map_err(|e| IbvConnectionBuildError::CompletionQueueCreationError(e))
    }

    fn create_pd(&self, context: &Context) -> Result<ProtectionDomain, IbvConnectionBuildError> {
        context
            .alloc_pd()
            .map_err(|e| IbvConnectionBuildError::ProtectionDomainCreationError(e))
    }

    fn create_qp(
        &self,
        pd: &ProtectionDomain,
        cq: &CompletionQueue,
    ) -> Result<PreparedQueuePair, IbvConnectionBuildError> {
        pd.create_qp(cq, cq, ::ibverbs::ibv_qp_type::IBV_QPT_RC)
            .map_err(|e| IbvConnectionBuildError::QueuePairCreationError(e))?
            .set_access(
                ::ibverbs::ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ::ibverbs::ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    | ::ibverbs::ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
            )
            .set_max_recv_wr(self.cq_params.capacity.min(u32::MAX as usize) as u32)
            .set_max_send_wr(self.cq_params.capacity.min(u32::MAX as usize) as u32)
            .build()
            .map_err(|e| IbvConnectionBuildError::QueuePairCreationError(e))
    }

    fn qp_endpoint(
        &self,
        qp: &PreparedQueuePair,
    ) -> Result<QueuePairEndpoint, IbvConnectionBuildError> {
        qp.endpoint()
            .map_err(|e| IbvConnectionBuildError::QueuePairCreationError(e))
    }

    fn inner_register_mrs(
        &self,
        pd: &ProtectionDomain,
    ) -> Result<Vec<(String, MemoryRegion)>, IbvConnectionBuildError> {
        let mut mr_endpoints = Vec::new();
        let mut registered_mr_ids = HashSet::new();
        for mr in &self.mrs.mrs {
            // Check id has not been previously registered
            if registered_mr_ids.contains(mr.id()) {
                return Err(IbvConnectionBuildError::MemoryRegionDuplicateRegister(
                    mr.id().to_string(),
                ));
            }
            let mr_endpoint = match mr {
                RdmaNamedMemory::Normal { id, ptr, length } => pd
                    .register(*ptr, *length)
                    .map_err(IbvConnectionBuildError::MemoryRegionRegisterError),
                RdmaNamedMemory::HugeTlb { id, ptr, length } => pd
                    .register_with_permissions(
                        *ptr,
                        *length,
                        ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
                            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
                            | ibv_access_flags::IBV_ACCESS_HUGETLB
                            | ibv_access_flags::IBV_ACCESS_ON_DEMAND,
                    )
                    .map_err(IbvConnectionBuildError::MemoryRegionRegisterError),
                RdmaNamedMemory::Dma { id, fd, length } => pd
                    .register_dmabuf(*fd, 0, *length, ibv_access_flags::IBV_ACCESS_REMOTE_READ)
                    .map_err(IbvConnectionBuildError::MemoryRegionRegisterError),
            }?;

            registered_mr_ids.insert(mr.id().to_string());
            mr_endpoints.push((mr.id().to_string(), mr_endpoint));
        }
        mr_endpoints.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(mr_endpoints)
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvPreparedConnection {
    local_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    qp: PreparedQueuePair,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: Rc<RefCell<CachedCompletionQueue>>,
    #[derivative(Debug = "ignore")]
    mrs: Vec<(String, MemoryRegion)>,
}

impl IbvPreparedConnection {
    pub fn endpoint(&self) -> IbvConnectionEndpoint {
        IbvConnectionEndpoint {
            qp_endpoint: self.local_qp_endpoint,
            mr_endpoints: self
                .mrs
                .iter()
                .map(|(id, mr)| (id.clone(), mr.remote()))
                .collect(),
        }
    }

    pub fn connect(
        self,
        connection_config: IbvConnectionEndpoint,
    ) -> Result<IbvConnection, IbvConnectionBuildError> {
        let qp = self
            .qp
            .handshake(connection_config.qp_endpoint)
            .map_err(|e| IbvConnectionBuildError::ConnectionError(e))?;

        let local_mrs = self
            .mrs
            .into_iter()
            .map(|(id, mr)| (id.clone(), IbvMemoryRegion::new(id, mr)))
            .collect();

        let remote_mrs = connection_config
            .mr_endpoints
            .into_iter()
            .map(|(id, rmr)| (id.clone(), IbvRemoteMemoryRegion::new(id, rmr)))
            .collect();

        Ok(IbvConnection {
            local_qp_endpoint: self.local_qp_endpoint,
            remote_qp_endpoint: connection_config.qp_endpoint,
            qp,
            _pd: self.pd,
            cq: self.cq,
            local_mrs,
            remote_mrs,
            next_wr_id: 0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbvConnectionEndpoint {
    qp_endpoint: QueuePairEndpoint,
    mr_endpoints: Vec<(String, RemoteMemoryRegion)>,
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
    #[derivative(Debug = "ignore")]
    local_mrs: HashMap<String, IbvMemoryRegion>,
    #[derivative(Debug = "ignore")]
    remote_mrs: HashMap<String, IbvRemoteMemoryRegion>,
    #[derivative(Debug = "ignore")]
    next_wr_id: u64,
}

#[derive(Clone, Debug)]
pub struct IbvMemoryRegion {
    mr: Arc<Named<MemoryRegion>>,
}

impl IbvMemoryRegion {
    fn new(name: impl Into<String>, mr: MemoryRegion) -> Self {
        Self {
            mr: Arc::new(Named::new(name, mr)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IbvRemoteMemoryRegion {
    mr: Arc<Named<RemoteMemoryRegion>>,
}

impl IbvRemoteMemoryRegion {
    fn new(name: impl Into<String>, mr: RemoteMemoryRegion) -> Self {
        Self {
            mr: Arc::new(Named::new(name, mr)),
        }
    }
}

impl IbvConnection {
    fn next_wr_id(&mut self) -> u64 {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;
        wr_id
    }
}

impl RdmaMemoryRegionConnection for IbvConnection {
    type MemoryRegion = IbvMemoryRegion;
}

impl RdmaRemoteMemoryRegionConnection for IbvConnection {
    type RemoteMemoryRegion = IbvRemoteMemoryRegion;
}

impl RdmaNamedMemoryRegionConnection for IbvConnection {
    fn local_mr(&self, id: impl AsRef<str>) -> Option<IbvMemoryRegion> {
        self.local_mrs.get(id.as_ref()).cloned()
    }
}

impl RdmaNamedRemoteMemoryRegionConnection for IbvConnection {
    fn remote_mr(&self, id: impl AsRef<str>) -> Option<IbvRemoteMemoryRegion> {
        self.remote_mrs.get(id.as_ref()).cloned()
    }
}

#[derive(Debug, Error)]
pub enum IbvPostError {
    #[error("Memory region does not associated to connection")]
    InvalidMemoryRegion,
    #[error("Remote memory region does not associated to connection")]
    InvalidRemoteMemoryRegion,
}

impl RdmaPostSendConnection for IbvConnection {
    type WorkRequest = IbvWorkRequest;
    type PostError = std::io::Error;

    fn post_send(
        &mut self,
        memory_region: &IbvMemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id();
        unsafe {
            self.qp.post_send(
                &[memory_region.mr.data.slice(memory_range)],
                wr_id,
                immediate_data,
            )
        }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }
}

impl RdmaPostReceiveConnection for IbvConnection {
    type WorkRequest = IbvWorkRequest;
    type PostError = std::io::Error;

    fn post_receive(
        &mut self,
        memory_region: &IbvMemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id();
        unsafe {
            self.qp
                .post_receive(&[memory_region.mr.data.slice(memory_range)], wr_id)
        }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }
}

impl RdmaPostWriteConnection for IbvConnection {
    type WorkRequest = IbvWorkRequest;
    type PostError = std::io::Error;

    fn post_write(
        &mut self,
        local_memory_region: &IbvMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &IbvRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        let local_mr_slice = local_memory_region.mr.data.slice(local_memory_range);
        let remote_mr_slice = remote_memory_region.mr.data.slice(remote_memory_range);

        let wr_id = self.next_wr_id();
        self.qp
            .post_write(&[local_mr_slice], remote_mr_slice, wr_id, immediate_data)?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }
}

impl RdmaPostReadConnection for IbvConnection {
    type WorkRequest = IbvWorkRequest;
    type PostError = std::io::Error;

    fn post_read(
        &mut self,
        local_memory_region: &IbvMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &IbvRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        let local_mr_slice = local_memory_region.mr.data.slice(local_memory_range);
        let remote_mr_slice = remote_memory_region.mr.data.slice(remote_memory_range);

        let wr_id = self.next_wr_id();
        self.qp
            .post_read(&[local_mr_slice], remote_mr_slice, wr_id)?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }
}

impl RdmaPostSendImmediateDataConnection for IbvConnection {
    type WorkRequest = IbvWorkRequest;
    type PostError = std::io::Error;

    fn post_send_immediate_data(
        &mut self,
        immediate_data: u32,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id();
        unsafe { self.qp.post_send(&[], wr_id, Some(immediate_data)) }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }
}

impl RdmaPostReceiveImmediateDataConnection for IbvConnection {
    type WorkRequest = IbvWorkRequest;
    type PostError = std::io::Error;

    fn post_receive_immediate_data(&mut self) -> Result<IbvWorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id();
        unsafe { self.qp.post_receive(&[], wr_id) }?;
        Ok(IbvWorkRequest::new(wr_id, self.cq.clone()))
    }
}

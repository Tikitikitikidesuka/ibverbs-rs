use crate::restructure::ibverbs::completion_queue::CachedCompletionQueue;
use crate::restructure::ibverbs::memory_region::{IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::restructure::ibverbs::work_completion::IbvWorkCompletion;
use crate::restructure::ibverbs::work_request::IbvWorkRequest;
use crate::restructure::rdma_connection::RdmaConnection;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, Context, PreparedQueuePair, ProtectionDomain, QueuePair, QueuePairEndpoint,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    mrs: Vec<BuilderMemoryRegion>,
}

#[derive(Debug)]
pub struct BuilderMemoryRegion {
    id: String,
    ptr: *mut u8,
    length: usize,
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
    pub fn register_mr(
        self,
        id: impl Into<String>,
        mem_ptr: *mut u8,
        mem_length: usize,
    ) -> IbvConnectionBuilder<BuilderIbvDeviceName, BuilderCqParams, BuilderMemoryRegions> {
        IbvConnectionBuilder {
            ibv_device_name: self.ibv_device_name,
            cq_params: self.cq_params,
            mrs: BuilderMemoryRegions {
                mrs: vec![BuilderMemoryRegion {
                    id: id.into(),
                    ptr: mem_ptr,
                    length: mem_length,
                }],
            },
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
        let mrs = self.register_mrs(&pd)?;

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
        ibverbs::devices()
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
        pd.create_qp(cq, cq, ibverbs::ibv_qp_type::IBV_QPT_RC)
            .map_err(|e| IbvConnectionBuildError::QueuePairCreationError(e))?
            .set_access(
                ibverbs::ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibverbs::ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    | ibverbs::ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
            )
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

    fn register_mrs(
        &self,
        pd: &ProtectionDomain,
    ) -> Result<Vec<(String, IbvMemoryRegion)>, IbvConnectionBuildError> {
        let mut mr_endpoints = Vec::new();
        let mut registered_mr_ids = HashSet::new();
        for mr in &self.mrs.mrs {
            // Check id has not been previously registered
            if !registered_mr_ids.contains(&mr.id) {
                let mr_endpoint = pd
                    .register(mr.ptr, mr.length)
                    .map_err(|e| IbvConnectionBuildError::MemoryRegionRegisterError(e))?;
                registered_mr_ids.insert(mr.id.clone());
                mr_endpoints.push((
                    mr.id.clone(),
                    IbvMemoryRegion {
                        length: mr.length,
                        mr: Arc::new(mr_endpoint),
                    },
                ));
            } else {
                return Err(IbvConnectionBuildError::MemoryRegionDuplicateRegister(
                    mr.id.clone(),
                ));
            }
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
    cq: Rc<RefCell<CachedCompletionQueue>>,
    mrs: Vec<(String, IbvMemoryRegion)>,
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

        let local_mrs = self.mrs.into_iter().collect();
        let remote_mrs = connection_config.mr_endpoints.into_iter().collect();

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
    local_mrs: HashMap<String, IbvMemoryRegion>,
    remote_mrs: HashMap<String, IbvRemoteMemoryRegion>,
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
    pub fn local_mr(&self, id: impl AsRef<str>) -> Option<IbvMemoryRegion> {
        self.local_mrs.get(id.as_ref()).cloned()
    }

    pub fn remote_mr(&self, id: impl AsRef<str>) -> Option<IbvRemoteMemoryRegion> {
        self.remote_mrs.get(id.as_ref()).cloned()
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

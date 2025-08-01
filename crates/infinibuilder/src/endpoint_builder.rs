use crate::IbBUnconnectedEndpoint;
use ibverbs::Context;
use ibverbs::ibv_qp_type::IBV_QPT_RC;
use std::io;
use crate::unsafe_slice::UnsafeSlice;

pub struct UnsetContext;
pub struct SetContext<'a>(&'a Context);

pub struct UnsetDataMemoryRegion;
pub struct SetDataMemoryRegion(UnsafeSlice);

pub struct UnsetCompletionQueueSize;
pub struct SetCompletionQueueSize(usize);

// Attribute order is important since Rust drops attributes in order of declaration
// QP must be destroyed before CQ
pub struct IbBEndpointBuilder<ContextStatus, DataMemoryRegionStatus, CompletionQueueSizeStatus> {
    context: ContextStatus,
    data_mr: DataMemoryRegionStatus,
    cq_size: CompletionQueueSizeStatus,
}

impl IbBEndpointBuilder<UnsetContext, UnsetDataMemoryRegion, UnsetCompletionQueueSize> {
    pub fn new() -> Self {
        Self {
            context: UnsetContext,
            data_mr: UnsetDataMemoryRegion,
            cq_size: UnsetCompletionQueueSize,
        }
    }
}

impl<DataMemoryRegionStatus, CompletionQueueSizeStatus>
    IbBEndpointBuilder<UnsetContext, DataMemoryRegionStatus, CompletionQueueSizeStatus>
{
    pub fn set_context(
        self,
        context: &Context,
    ) -> IbBEndpointBuilder<SetContext, DataMemoryRegionStatus, CompletionQueueSizeStatus> {
        IbBEndpointBuilder {
            context: SetContext(context),
            data_mr: self.data_mr,
            cq_size: self.cq_size,
        }
    }
}

impl<ContextStatus, CompletionQueueSizeStatus>
    IbBEndpointBuilder<ContextStatus, UnsetDataMemoryRegion, CompletionQueueSizeStatus>
{
    /// SAFETY: Takes the memory region as &[u8] and decouples it from the reference meaning
    /// the memory must be ensured by the user to live more than the IbBEndopint
    pub unsafe fn set_data_memory_region(
        self,
        memory: &[u8],
    ) -> IbBEndpointBuilder<ContextStatus, SetDataMemoryRegion, CompletionQueueSizeStatus> {
        IbBEndpointBuilder {
            context: self.context,
            data_mr: SetDataMemoryRegion(unsafe { UnsafeSlice::new(memory) }),
            cq_size: self.cq_size,
        }
    }
}

impl<ContextStatus, DataMemoryRegionStatus>
    IbBEndpointBuilder<ContextStatus, DataMemoryRegionStatus, UnsetCompletionQueueSize>
{
    pub fn set_completion_queue_size(
        self,
        size: usize,
    ) -> IbBEndpointBuilder<ContextStatus, DataMemoryRegionStatus, SetCompletionQueueSize> {
        IbBEndpointBuilder {
            context: self.context,
            data_mr: self.data_mr,
            cq_size: SetCompletionQueueSize(size),
        }
    }
}

impl<'a> IbBEndpointBuilder<SetContext<'_>, SetDataMemoryRegion, SetCompletionQueueSize> {
    pub fn build(self) -> io::Result<IbBUnconnectedEndpoint> {
        let context = self.context.0;
        let cq_size = self.cq_size.0;
        let cq = context.create_cq(cq_size as i32, 0)?;
        let pd = context.alloc_pd()?;
        let prepared_qp = pd.create_qp(&cq, &cq, IBV_QPT_RC)?.build()?;
        let data_mr = pd.register(self.data_mr.0)?;
        let endpoint = prepared_qp.endpoint()?;

        Ok(IbBUnconnectedEndpoint {
            prepared_qp,
            cq,
            cq_size,
            pd,
            data_mr,
            endpoint,
        })
    }
}

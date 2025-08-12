use crate::IbBConnectedNode;
use crate::data_transmission::UnsafeSlice;
use ibverbs::Context;
use ibverbs::ibv_qp_type::IBV_QPT_RC;
use std::io;
use std::marker::PhantomData;

pub struct UnsetContext;
pub struct SetContext<'a>(&'a Context);

pub struct UnsetDataMemoryRegion;
pub struct SetDataMemoryRegion(UnsafeSlice);

pub struct UnsetCompletionQueueSize;
pub struct SetCompletionQueueSize(usize);

// Attribute order is important since Rust drops attributes in order of declaration
// QP must be destroyed before CQ
pub struct IbBConnectedNodeBuilder<
    ContextStatus,
    DataMemoryRegionStatus,
    CompletionQueueSizeStatus,
    DataTransmissionBackend,
    SyncBackend,
> {
    context: ContextStatus,
    data_mr: DataMemoryRegionStatus,
    cq_size: CompletionQueueSizeStatus,
    _dtb: PhantomData<DataTransmissionBackend>,
    _sb: PhantomData<SyncBackend>,
}

impl<DataTransmissionBackend, SyncBackend>
    IbBConnectedNodeBuilder<
        UnsetContext,
        UnsetDataMemoryRegion,
        UnsetCompletionQueueSize,
        DataTransmissionBackend,
        SyncBackend,
    >
{
    pub fn new() -> Self {
        Self {
            context: UnsetContext,
            data_mr: UnsetDataMemoryRegion,
            cq_size: UnsetCompletionQueueSize,
            _dtb: PhantomData::default(),
            _sb: PhantomData::default(),
        }
    }
}

impl<DataMemoryRegionStatus, CompletionQueueSizeStatus, DataTransmissionBackend, SyncBackend>
    IbBConnectedNodeBuilder<
        UnsetContext,
        DataMemoryRegionStatus,
        CompletionQueueSizeStatus,
        DataTransmissionBackend,
        SyncBackend,
    >
{
    pub fn set_context(
        self,
        context: &Context,
    ) -> IbBConnectedNodeBuilder<
        SetContext,
        DataMemoryRegionStatus,
        CompletionQueueSizeStatus,
        DataTransmissionBackend,
        SyncBackend,
    > {
        IbBConnectedNodeBuilder {
            context: SetContext(context),
            data_mr: self.data_mr,
            cq_size: self.cq_size,
            _dtb: self._dtb,
            _sb: self._sb,
        }
    }
}

impl<ContextStatus, CompletionQueueSizeStatus, DataTransmissionBackend, SyncBackend>
    IbBConnectedNodeBuilder<
        ContextStatus,
        UnsetDataMemoryRegion,
        CompletionQueueSizeStatus,
        DataTransmissionBackend,
        SyncBackend,
    >
{
    /// SAFETY: Takes the memory region as &[u8] and decouples it from the reference meaning
    /// the memory must be ensured by the user to live more than the IbBEndopint
    pub unsafe fn set_data_memory_region(
        self,
        memory: &[u8],
    ) -> IbBConnectedNodeBuilder<
        ContextStatus,
        SetDataMemoryRegion,
        CompletionQueueSizeStatus,
        DataTransmissionBackend,
        SyncBackend,
    > {
        IbBConnectedNodeBuilder {
            context: self.context,
            data_mr: SetDataMemoryRegion(unsafe { UnsafeSlice::new(memory) }),
            cq_size: self.cq_size,
            _dtb: self._dtb,
            _sb: self._sb,
        }
    }
}

impl<ContextStatus, DataMemoryRegionStatus, DataTransmissionBackend, SyncBackend>
    IbBConnectedNodeBuilder<
        ContextStatus,
        DataMemoryRegionStatus,
        UnsetCompletionQueueSize,
        DataTransmissionBackend,
        SyncBackend,
    >
{
    pub fn set_completion_queue_size(
        self,
        size: usize,
    ) -> IbBConnectedNodeBuilder<
        ContextStatus,
        DataMemoryRegionStatus,
        SetCompletionQueueSize,
        DataTransmissionBackend,
        SyncBackend,
    > {
        IbBConnectedNodeBuilder {
            context: self.context,
            data_mr: self.data_mr,
            cq_size: SetCompletionQueueSize(size),
            _dtb: self._dtb,
            _sb: self._sb,
        }
    }
}

impl<'a, DataTransmissionBackend, SyncBackend>
    IbBConnectedNodeBuilder<
        SetContext<'_>,
        SetDataMemoryRegion,
        SetCompletionQueueSize,
        DataTransmissionBackend,
        SyncBackend,
    >
{
    pub fn build(self) -> io::Result<IbBConnectedNode<DataTransmissionBackend, SyncBackend>> {
        let context = self.context.0;
        let cq_size = self.cq_size.0;
        let cq = context.create_cq(cq_size as i32, 0)?;
        let pd = context.alloc_pd()?;
        let prepared_qp = pd.create_qp(&cq, &cq, IBV_QPT_RC)?.build()?;
        let data_mr = pd.register(self.data_mr.0)?;
        let endpoint = prepared_qp.endpoint()?;

        Ok(IbBConnectedNode {
            prepared_qp,
            cq,
            cq_size,
            pd,
            data_mr,
            endpoint,
        })
    }
}

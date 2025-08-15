use ibverbs::Context;
use ibverbs::ibv_qp_type::IBV_QPT_RC;
use std::io;
use crate::synchronization::backend::IbBSynchronizationMasterBackend;

// Status types for the builder
pub struct UnsetContext;
pub struct SetContext<'a>(&'a Context);
pub struct UnsetNodes;
pub struct SetNodes(u32);
pub struct UnsetCompletionQueueSize;
pub struct SetCompletionQueueSize(usize);

// Attribute order is important since Rust drops attributes in order of declaration
// QP must be destroyed before CQ
pub struct IbBSynchronizationMasterBackendBuilder<
    ContextStatus,
    NodesStatus,
    CompletionQueueSizeStatus,
> {
    context: ContextStatus,
    nodes: NodesStatus,
    cq_size: CompletionQueueSizeStatus,
}

impl IbBSynchronizationMasterBackendBuilder<UnsetContext, UnsetNodes, UnsetCompletionQueueSize> {
    pub fn new() -> Self {
        Self {
            context: UnsetContext,
            nodes: UnsetNodes,
            cq_size: UnsetCompletionQueueSize,
        }
    }
}

impl<NodesStatus, CompletionQueueSizeStatus>
    IbBSynchronizationMasterBackendBuilder<UnsetContext, NodesStatus, CompletionQueueSizeStatus>
{
    pub fn set_context(
        self,
        context: &Context,
    ) -> IbBSynchronizationMasterBackendBuilder<SetContext, NodesStatus, CompletionQueueSizeStatus>
    {
        IbBSynchronizationMasterBackendBuilder {
            context: SetContext(context),
            nodes: self.nodes,
            cq_size: self.cq_size,
        }
    }
}

impl<ContextStatus, CompletionQueueSizeStatus>
    IbBSynchronizationMasterBackendBuilder<ContextStatus, UnsetNodes, CompletionQueueSizeStatus>
{
    pub fn set_nodes(
        self,
        nodes: u32,
    ) -> IbBSynchronizationMasterBackendBuilder<ContextStatus, SetNodes, CompletionQueueSizeStatus>
    {
        IbBSynchronizationMasterBackendBuilder {
            context: self.context,
            nodes: SetNodes(nodes),
            cq_size: self.cq_size,
        }
    }
}

impl<ContextStatus, NodesStatus>
    IbBSynchronizationMasterBackendBuilder<ContextStatus, NodesStatus, UnsetCompletionQueueSize>
{
    pub fn set_completion_queue_size(
        self,
        size: usize,
    ) -> IbBSynchronizationMasterBackendBuilder<ContextStatus, NodesStatus, SetCompletionQueueSize>
    {
        IbBSynchronizationMasterBackendBuilder {
            context: self.context,
            nodes: self.nodes,
            cq_size: SetCompletionQueueSize(size),
        }
    }
}

impl IbBSynchronizationMasterBackendBuilder<SetContext<'_>, SetNodes, SetCompletionQueueSize> {
    pub fn build(self) -> io::Result<IbBSynchronizationMasterBackend> {
        let context = self.context.0;
        let nodes = self.nodes.0;
        let cq_size = self.cq_size.0;

        // Create completion queue
        let cq = context.create_cq(cq_size as i32, 0)?;

        // Allocate protection domain
        let pd = context.alloc_pd()?;

        // Create memory region for synchronization flags (one u32 per node)
        let sync_mr = pd.allocate(nodes as usize)?;

        // Create queue pair
        let prepared_qp = pd.create_qp(&cq, &cq, IBV_QPT_RC)?.build()?;

        // Get endpoint
        let endpoint = prepared_qp.endpoint()?;

        Ok(IbBSynchronizationMasterBackend {
            prepared_qp,
            cq,
            cq_size,
            pd,
            sync_mr,
            endpoint,
        })
    }
}

// Convenience method for building with default completion queue size
impl IbBSynchronizationMasterBackendBuilder<SetContext<'_>, SetNodes, UnsetCompletionQueueSize> {
    pub fn build_with_default_cq_size(self) -> io::Result<IbBSynchronizationMasterBackend> {
        // Default CQ size could be based on nodes or a reasonable constant
        let default_cq_size = std::cmp::max(64, self.nodes.0 as usize * 2);
        self.set_completion_queue_size(default_cq_size).build()
    }
}

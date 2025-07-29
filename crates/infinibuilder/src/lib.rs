use ibverbs::ibv_qp_type::IBV_QPT_RC;
use ibverbs::{
    CompletionQueue, Context, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint,
};

pub struct SetDataMemoryRegion<'a>(MemoryRegion<&'a mut [u8]>);
pub struct UnsetDataMemoryRegion;

// Attribute order is important since Rust drops attributes from last to first!!
pub struct EndpointBuilder<DataMemoryRegionStatus> {
    data_mr: DataMemoryRegionStatus,
    endpoint: QueuePairEndpoint,
    prepared_qp: PreparedQueuePair,
    pd: ProtectionDomain,
    cq: CompletionQueue,
}

impl EndpointBuilder<UnsetDataMemoryRegion> {
    pub fn new(context: &Context) -> Self {
        let cq = context.create_cq(16, 0).unwrap();
        let pd = context.alloc_pd().unwrap();
        let prepared_qp = pd.create_qp(&cq, &cq, IBV_QPT_RC).unwrap().build().unwrap();
        let endpoint = prepared_qp.endpoint().unwrap();

        Self {
            cq,
            pd,
            prepared_qp,
            endpoint,
            data_mr: UnsetDataMemoryRegion,
        }
    }
}

impl EndpointBuilder<UnsetDataMemoryRegion> {
    pub fn set_data_memory_region(self, buffer: &mut [u8]) -> EndpointBuilder<SetDataMemoryRegion> {
        let data_mr = self.pd.register(buffer).unwrap();
        EndpointBuilder {
            cq: self.cq,
            pd: self.pd,
            prepared_qp: self.prepared_qp,
            endpoint: self.endpoint,
            data_mr: SetDataMemoryRegion(data_mr),
        }
    }
}

impl<'a> EndpointBuilder<SetDataMemoryRegion<'a>> {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    pub fn connect(self, endpoint: QueuePairEndpoint) -> Endpoint<'a> {
        let qp = self.prepared_qp.handshake(endpoint).unwrap();

        Endpoint {
            cq: self.cq,
            pd: self.pd,
            qp,
            endpoint: self.endpoint,
            data_mr: self.data_mr.0,
        }
    }
}

// Attribute order is important since Rust drops attributes from last to first!!
pub struct Endpoint<'a> {
    cq: CompletionQueue,
    pd: ProtectionDomain,
    data_mr: MemoryRegion<&'a mut [u8]>,
    qp: QueuePair,
    endpoint: QueuePairEndpoint,
}

impl Endpoint<'_> {
    /*
    pub fn post_send();
    pub fn post_receive();
    pub fn poll_send();
    pub fn poll_receive();
    pub fn wait_send();
    pub fn wait_receive();
    */
}

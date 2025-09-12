use crate::component::UnconnectedComponent;

pub mod centralized;

pub trait SyncComponent {
    fn wait_barrier(&mut self) -> std::io::Result<()>;
}

pub trait UnconnectedSyncComponent: UnconnectedComponent {
    type SyncComponent: SyncComponent;

    fn new(
        context: ibverbs::Context,
        sync_idx: usize,
        num_nodes: usize,
    ) -> std::io::Result<Self> where Self: Sized;
}

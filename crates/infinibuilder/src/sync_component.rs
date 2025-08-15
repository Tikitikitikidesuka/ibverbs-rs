pub trait SyncComponent {
    fn wait_barrier(&mut self) -> std::io::Result<()>;
}
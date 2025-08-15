pub trait IbBNodeSync {
    fn wait_barrier(&mut self) -> std::io::Result<()>;
}

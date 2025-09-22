pub trait Connect {
    type ConnectionConfig;
    type Connected;

    fn connection_config(&self) -> Self::ConnectionConfig;
    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected>;
}

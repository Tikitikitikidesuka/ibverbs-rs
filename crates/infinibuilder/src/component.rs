pub trait UnconnectedComponent {
    type ConnectionOutputConfig;
    type ConnectionInputConfig;
    type ConnectedComponent;

    fn connection_config(&self) -> Self::ConnectionOutputConfig;

    fn connect(
        self,
        connection_config: Self::ConnectionInputConfig,
    ) -> std::io::Result<Self::ConnectedComponent>;
}

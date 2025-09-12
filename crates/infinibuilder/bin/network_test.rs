use infinibuilder::network::{IBNetwork, IBNetworkBuilder, IBNodeConfig, IBNodeRole};

fn main() {
    let network = network();
    TcpE
}

fn network() -> IBNetwork<&'static str> {
    let mut network_builder = IBNetworkBuilder::new();
    network_builder.insert_node("RU0", IBNodeConfig {
        role: IBNodeRole::Sender,
        hostname: "tdeb01".to_string(),
        port: 8000,
    });
    network_builder.insert_node("RU1", IBNodeConfig {
        role: IBNodeRole::Sender,
        hostname: "tdeb02".to_string(),
        port: 8000,
    });
    network_builder.insert_node("RU2", IBNodeConfig {
        role: IBNodeRole::Sender,
        hostname: "tdeb03".to_string(),
        port: 8000,
    });
    network_builder.insert_node("BU0", IBNodeConfig {
        role: IBNodeRole::Sender,
        hostname: "tdeb05".to_string(),
        port: 8000,
    });
    network_builder.insert_node("BU1", IBNodeConfig {
        role: IBNodeRole::Sender,
        hostname: "tdeb06".to_string(),
        port: 8000,
    });
    network_builder.insert_node("BU2", IBNodeConfig {
        role: IBNodeRole::Sender,
        hostname: "tdeb07".to_string(),
        port: 8000,
    });
    network_builder.build()
}
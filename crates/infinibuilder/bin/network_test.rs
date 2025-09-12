use infinibuilder::config_exchange::{TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig};
use infinibuilder::network::{IBNetwork, IBNetworkBuilder, IBNodeBuilderConfig, IBNodeRole};
use std::env;
use std::time::Duration;

fn main() {
    RunParams::from_args(env::args()).unwrap();
    let network = network();
    let exchanger_network = TcpExchangerNetworkConfig::from_network(network).unwrap();
    let exchanged = TcpExchanger::await_exchange_network_config(
        0,
        &"HELLO".to_owned(),
        &exchanger_network,
        &exchanger_config(),
    );
}

#[derive(Debug)]
struct RunParams {
    node_id: String,
    network_file: String,
}

impl RunParams {
    fn from_args<I, T>(mut args: I) -> Result<Self, String>
    where
        I: Iterator<Item = T>,
        T: Into<String>,
    {
        args.next(); // skip program name

        let node_id = args.next().ok_or("Missing node_id argument")?.into();
        let network_file = args.next().ok_or("Missing network_file argument")?.into();

        if args.next().is_some() {
            return Err("Too many arguments provided".into());
        }

        Ok(RunParams {
            node_id,
            network_file,
        })
    }
}

fn network() -> IBNetwork<&'static str> {
    let mut network_builder = IBNetworkBuilder::new();
    network_builder.insert_node(
        "RU0",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb01".to_string(),
            port: 8000,
        },
    );
    network_builder.insert_node(
        "RU1",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb02".to_string(),
            port: 8000,
        },
    );
    network_builder.insert_node(
        "RU2",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb03".to_string(),
            port: 8000,
        },
    );
    network_builder.insert_node(
        "BU0",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb05".to_string(),
            port: 8000,
        },
    );
    network_builder.insert_node(
        "BU1",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb06".to_string(),
            port: 8000,
        },
    );
    network_builder.insert_node(
        "BU2",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb07".to_string(),
            port: 8000,
        },
    );
    network_builder.build()
}

fn exchanger_config() -> TcpExchangerConfig {
    TcpExchangerConfig {
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    }
}

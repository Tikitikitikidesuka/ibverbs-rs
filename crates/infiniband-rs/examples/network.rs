use infiniband_rs::network::host::{IbvNetworkHost, IbvNetworkRank};
use infiniband_rs::network::network_config::IbvUncheckedNetworkConfig;
use infiniband_rs::network::tcp_exchanger::{TcpExchangeConfig, TcpExchanger};
use std::{env, fs};

const CONFIG_FILE: &str = "network.json";
const RANK: IbvNetworkRank = 3;

fn main() {
    let cwd = env::current_dir().unwrap();
    let path = cwd.join("network.json");
    let json_network = fs::read_to_string(&path).unwrap();
    let network_config = serde_json::from_str::<IbvUncheckedNetworkConfig>(&json_network)
        .unwrap()
        .check()
        .unwrap();

    let host = IbvNetworkHost::builder()
        .rank(RANK)
        .config(&network_config)
        .build()
        .unwrap();
    let endpoint = host.endpoint();

    let remote_endpoints = TcpExchanger::await_exchange_all(
        RANK,
        &network_config,
        &endpoint,
        &TcpExchangeConfig::default(),
    ).unwrap();

    println!("{remote_endpoints:?}")
}

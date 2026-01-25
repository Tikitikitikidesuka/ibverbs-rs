use infiniband_rs::ibverbs::devices::open_device;
use infiniband_rs::network::Node;
use infiniband_rs::network::config::{NetworkConfig, NodeConfig, RawNetworkConfig};
use infiniband_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::{env, fs, process};

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <network.json> <rank>", args[0]);
        process::exit(1);
    }

    let network_path = &args[1];
    let rank: usize = args[2].parse().unwrap();

    let json_network = fs::read_to_string(network_path).unwrap();

    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .build()
        .unwrap();

    let node_config: &NodeConfig = network_config.get(rank).unwrap();

    let ctx = open_device(&node_config.ibdev).unwrap();

    let node = Node::builder()
        .context(&ctx)
        .rank(node_config.rankid)
        .world_size(network_config.world_size())
        .build()
        .unwrap();

    let endpoint = node.endpoint();

    let remote_endpoints =
        Exchanger::await_exchange_all(rank, &network_config, &endpoint, &ExchangeConfig::default())
            .unwrap();

    let remote_endpoints = node.gather_endpoints(remote_endpoints).unwrap();

    let node = node.handshake(remote_endpoints).unwrap();
}

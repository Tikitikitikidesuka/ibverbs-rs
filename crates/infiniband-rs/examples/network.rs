use infiniband_rs::network::network_config::UncheckedNetworkConfig;
use infiniband_rs::network::node::{Node, Rank};
use infiniband_rs::network::prepared_host::NodeGatherEndpoint;
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
    let rank: Rank = args[2].parse().unwrap();

    let json_network = fs::read_to_string(network_path).unwrap();

    let network_config = serde_json::from_str::<UncheckedNetworkConfig>(&json_network)
        .unwrap()
        .check()
        .unwrap();

    let prepared_host = Node::builder()
        .rank(rank)
        .config(&network_config)
        .build()
        .unwrap();

    let endpoint = prepared_host.endpoint();

    let scattered_remote_endpoints =
        Exchanger::await_exchange_all(rank, &network_config, &endpoint, &ExchangeConfig::default())
            .unwrap();

    let remote_endpoints = NodeGatherEndpoint::gather(rank, scattered_remote_endpoints).unwrap();

    let mut host = prepared_host.handshake(remote_endpoints).unwrap();

    if host.rank() == 0 || host.rank() == 1 {
        let mut mem = [0u8; 512];
        let mr = host.register_mr(&mut mem).unwrap();
        let se0 = mr.prepare_scatter_element(&mem);
        host.send(2, &[se0]).unwrap_or_else(|e| panic!("Error: {e}"));
    } else {
        /*
        let mut mem = [0u8; 1024];
        let mr = host.register_mr(&mut mem).unwrap();
        let ge0 = mr.prepare_gather_element(&mut mem[0..512]);
        host.receive(0, &[ge0]).unwrap();
        let ge1 = mr.prepare_gather_element(&mut mem[512..1024]);
        host.receive(1, &[ge1]).unwrap();
        */
    }
}

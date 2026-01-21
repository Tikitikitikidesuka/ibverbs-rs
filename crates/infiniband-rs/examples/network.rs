use infiniband_rs::network::network_config::UncheckedNetworkConfig;
use infiniband_rs::network::node::{Node, Rank};
use infiniband_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::{env, fs, process};
use infiniband_rs::network::prepared_node::NodeGatherEndpoint;

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

    match host.rank() {
        0 => {
            let mut mem = [0u8; 8];
            println!("Mem before: {mem:?}");
            let mr = host.register_mr(&mut mem).unwrap();
            let ge0 = mr.prepare_gather_element(&mut mem[0..4]);
            host.receive(1,  &mut [ge0]).unwrap();
            let ge1 = mr.prepare_gather_element(&mut mem[4..8]);
            host.receive(2, &mut [ge1]).unwrap();
            println!("Mem after: {mem:?}");
        }
        1 => {
            let mut mem = [1u8; 4];
            let mr = host.register_mr(&mut mem).unwrap();
            let se0 = mr.prepare_scatter_element(&mem);
            host.send(0, &[se0])
                .unwrap_or_else(|e| panic!("Error: {e}"));
        }
        2 => {
            let mut mem = [2u8; 4];
            let mr = host.register_mr(&mut mem).unwrap();
            let se0 = mr.prepare_scatter_element(&mem);
            host.send(0, &[se0])
                .unwrap_or_else(|e| panic!("Error: {e}"));
        }
        _ => {
            println!("Invalid rank: {rank}");
        }
    }
}

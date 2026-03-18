use ibverbs_rs::ibverbs;
use ibverbs_rs::network::Node;
use ibverbs_rs::network::barrier::BarrierAlgorithm;
use ibverbs_rs::network::config::{NodeConfig, RawNetworkConfig};
use ibverbs_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::time::Duration;
use std::{env, fs, process};

const DEVICE: &str = "mlx5_0";

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

    let ctx = ibverbs::open_device(DEVICE).unwrap();
    let pd = ctx.allocate_pd().unwrap();

    let node = Node::builder()
        .pd(&pd)
        .rank(node_config.rankid)
        .world_size(network_config.world_size())
        .barrier(BarrierAlgorithm::Centralized)
        .build()
        .unwrap();

    let endpoint = node.endpoint();

    let remote_endpoints =
        Exchanger::await_exchange_all(rank, &network_config, &endpoint, &ExchangeConfig::default())
            .unwrap();

    let remote_endpoints = node.gather_endpoints(remote_endpoints).unwrap();

    let mut node = node.handshake(remote_endpoints).unwrap();

    for _ in 0..3 {
        println!("Press enter...");
        let mut buffer = String::new();
        std::io::stdin()
            .read_line(&mut buffer)
            .expect("Failed to read line");
        println!("Barrier start!");
        match node.rank() {
            0 => {
                node.barrier(&[0, 1, 2], Duration::from_millis(10000))
                    .unwrap();
            }
            1 => {
                node.barrier(&[0, 1, 2], Duration::from_millis(10000))
                    .unwrap();
            }
            2 => {
                node.barrier(&[0, 1, 2], Duration::from_millis(10000))
                    .unwrap();
            }
            _ => {
                println!("Invalid rank: {rank}");
            }
        }
        println!("Barrier end!");
    }
}

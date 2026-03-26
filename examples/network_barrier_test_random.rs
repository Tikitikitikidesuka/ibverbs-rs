use ibverbs_rs::ibverbs;
use ibverbs_rs::network::{
    BarrierAlgorithm, ExchangeConfig, Exchanger, Node, NodeConfig, RawNetworkConfig,
};
use log::LevelFilter::Debug;
use rand::RngExt;
use simple_logger::SimpleLogger;
use std::io::Read;
use std::time::Duration;
use std::{env, fs, process, thread};

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
        .barrier(BarrierAlgorithm::Dissemination)
        .world_size(network_config.world_size())
        .build()
        .unwrap();

    let endpoint = node.endpoint();

    let remote_endpoints =
        Exchanger::await_exchange_all(rank, &network_config, &endpoint, &ExchangeConfig::default())
            .unwrap();

    let remote_endpoints = node.gather_endpoints(remote_endpoints).unwrap();

    let mut node = node.handshake(remote_endpoints).unwrap();

    let mut rng = rand::rng();
    let all_nodes: Vec<_> = (0..node.world_size()).collect();

    println!("Press a key...");
    std::io::stdin().read(&mut []).expect("Failed to read line");

    for i in 0..10000 {
        if node.rank() == 1 {
            let delay_ms = rng.random_range(10..10000000); // e.g., 10-1000ns
            thread::sleep(Duration::from_nanos(delay_ms));
        }

        node.barrier(all_nodes.as_slice(), Duration::from_millis(10000))
            .unwrap();
        println!("Waited barrier {i} successfully");
    }
}

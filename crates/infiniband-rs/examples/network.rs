use infiniband_rs::ibverbs;
use infiniband_rs::multi_channel::work_request::{PeerReceiveWorkRequest, PeerSendWorkRequest};
use infiniband_rs::network::Node;
use infiniband_rs::network::config::{NodeConfig, RawNetworkConfig};
use infiniband_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
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
        .build()
        .unwrap();

    let endpoint = node.endpoint();

    let remote_endpoints =
        Exchanger::await_exchange_all(rank, &network_config, &endpoint, &ExchangeConfig::default())
            .unwrap();

    let remote_endpoints = node.gather_endpoints(remote_endpoints).unwrap();

    let mut node = node.handshake(remote_endpoints).unwrap();

    match node.rank() {
        0 => {
            let mut mem = [0u8; 8];
            println!("Mem before: {mem:?}");
            let mr = node.pd().register_local_mr_slice(&mem).unwrap();
            node.receive(PeerReceiveWorkRequest::new(
                1,
                &mut [mr.scatter_element(&mut mem[0..4])],
            ))
            .unwrap();
            node.receive(PeerReceiveWorkRequest::new(
                2,
                &mut [mr.scatter_element(&mut mem[4..8])],
            ))
            .unwrap();
            println!("Mem after: {mem:?}");
        }
        1 => {
            let mem = [1u8; 4];
            let mr = node.pd().register_local_mr_slice(&mem).unwrap();
            node.send(PeerSendWorkRequest::new(0, &[mr.gather_element(&mem)]))
                .unwrap_or_else(|e| panic!("Error: {e}"));
        }
        2 => {
            let mem = [2u8; 4];
            let mr = node.pd().register_local_mr_slice(&mem).unwrap();
            node.send(PeerSendWorkRequest::new(0, &[mr.gather_element(&mem)]))
                .unwrap_or_else(|e| panic!("Error: {e}"));
        }
        _ => {
            println!("Invalid rank: {rank}");
        }
    }
}

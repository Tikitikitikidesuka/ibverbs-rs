use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::restructure::barrier::centralized::RdmaNetworkCentralizedBarrier;
use infinibuilder::restructure::ibverbs::network_node::{
    IbvNetworkNodeBuilder, IbvNetworkNodeEndpoint,
};
use infinibuilder::restructure::rdma_network_node::RdmaNetworkNode;
use infinibuilder::restructure::tcp_exchanger::{TcpExchangeConfig, TcpExchanger};
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;
use std::{env, fs};
use infinibuilder::restructure::barrier::binary_tree::RdmaNetworkBinaryTreeBarrier;

fn main() {
    let args = parse_args();
    let rank_id: usize = args.rank_id;

    let json_network = fs::read_to_string(args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(args.num_nodes)
        .validate()
        .unwrap();

    let node_config = network_config.get(rank_id).unwrap();

    let prepared_node = IbvNetworkNodeBuilder::new()
        .ibv_device(&node_config.ibdev)
        .cq_params(32, 512)
        .barrier(RdmaNetworkBinaryTreeBarrier::new())
        .num_connections(network_config.len())
        .rank_id(rank_id)
        .build()
        .unwrap();

    let endpoint = prepared_node.endpoint();
    println!("Endpoint created...");

    println!("Beginning exchange...");
    let exchanged_endpoints = TcpExchanger::await_exchange_all(
        rank_id,
        &network_config,
        &endpoint,
        &TcpExchangeConfig::default(),
    )
    .unwrap();
    println!("Exchange done...");

    let remote_endpoint = IbvNetworkNodeEndpoint::gather(rank_id, exchanged_endpoints).unwrap();

    let mut node = prepared_node.connect(remote_endpoint).unwrap();

    println!("{node:?}");

    print!("Press Enter to enter barrier...");
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    node.barrier(&node.group_all(), Duration::from_millis(10000))
        .unwrap();
    println!("Barrier 1 done!!!\n\n");

    print!("Press Enter to enter barrier...");
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    node.barrier(&node.group_all(), Duration::from_millis(10000))
        .unwrap();
    println!("Barrier 2 done!!!\n\n");

    print!("Press Enter to enter barrier...");
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    node.barrier(&node.group_all(), Duration::from_millis(10000))
        .unwrap();
    println!("Barrier 3 done!!!\n\n");
}

struct Args {
    rank_id: usize,
    num_nodes: usize,
    config_file: String,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} config_file num_nodes rank_id", args[0]);
        std::process::exit(1);
    }

    let config_file = args[1].parse().unwrap();
    let num_nodes = usize::from_str(args[2].as_str()).unwrap();
    let rank_id = usize::from_str(args[3].as_str()).unwrap();

    Args {
        rank_id,
        num_nodes,
        config_file,
    }
}

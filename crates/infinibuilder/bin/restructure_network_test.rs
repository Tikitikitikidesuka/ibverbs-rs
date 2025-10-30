use infinibuilder::barrier::centralized::CentralizedBarrier;
use infinibuilder::ibverbs::init::create_ibv_network_node;
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_network_node::{RdmaBarrierNetworkNode, RdmaGroupNetworkNode};
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;
use std::{env, fs};
use infinibuilder::barrier::dissemination::DisseminationBarrier;

fn main() {
    const TRANSPORT_MR_ID: &str = "transport";

    let args = parse_args();
    let rank_id: usize = args.rank_id;

    let json_network = fs::read_to_string(args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(args.num_nodes);

    let mut memory = vec![0u8; 1024];
    let transport_mrs = Vec::<(String, *mut u8, usize)>::new();

    let mut node = create_ibv_network_node(
        rank_id,
        32,
        512,
        network_config,
        transport_mrs, //vec![(TRANSPORT_MR_ID, memory.as_mut_ptr(), memory.len())],
        DisseminationBarrier::new(),
    )
    .unwrap();

    /*
    if rank_id != 0 {
        print!("Press Enter to enter barrier...");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        node.barrier(
            &node
                .self_group(|rank_id| vec![1, 2, 3].contains(rank_id))
                .unwrap(),
            Duration::from_millis(10000),
        )
        .unwrap();
        println!("Barrier 1 done!!!\n\n");
    }

    if rank_id != 1 {
        print!("Press Enter to enter barrier...");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        node.barrier(
            &node
                .self_group(|rank_id| vec![0, 2, 3].contains(rank_id))
                .unwrap(),
            Duration::from_millis(10000),
        )
        .unwrap();
        println!("Barrier 2 done!!!\n\n");
    }
    */

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

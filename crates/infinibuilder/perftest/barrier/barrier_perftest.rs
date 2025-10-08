use BarrierAlgorithm::*;
use clap::Parser;
use infinibuilder::connect::Connect;
use infinibuilder::network::{ConnectedNetworkNode, NetworkNodeConnectionConfig, NetworkOp};
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::new_tcp_exchanger::{TcpExchangeConfig, TcpExchanger};
use infinibuilder::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use infinibuilder::synchronization::binary::BinaryTreeSync;
use infinibuilder::synchronization::centralized::CentralizedSync;
use infinibuilder::synchronization::dissemination::DisseminationSync;
use std::fs;
use tokio::time::Instant;

fn main() {
    simple_logger::init().unwrap();

    let args = Args::parse();
    let json_network = fs::read_to_string(&args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(args.num_nodes)
        .validate()
        .unwrap();
    let mut mem = Box::new([0u8; 1024]);

    let node = unsafe {
        ConnectedNetworkNode::new_ibv_simple_unit_network_node::<64, 64>(
            args.rank_id,
            &network_config,
            &mut *mem as *mut u8,
            mem.len(),
        )
    }
    .unwrap();

    let out_conn_config = node.connection_config();

    println!("Exchanging...");
    let exchanged = TcpExchanger::await_exchange_all(
        args.rank_id,
        &network_config,
        &out_conn_config,
        &TcpExchangeConfig::default(),
    ).unwrap();
    println!("Exchanged!");

    let in_conn_config =
        NetworkNodeConnectionConfig::gather(args.rank_id, exchanged.as_slice()).unwrap();

    let mut node = node.connect(in_conn_config).unwrap();
    println!("Node ready!!!");

    if let Some(iters) = args.iters {
        for _ in 0..iters {
            barrier_batch(&mut node, &args);
        }
    } else {
        loop {
            barrier_batch(&mut node, &args);
        }
    }
}

fn barrier_batch<T: RdmaSendRecv + RdmaRendezvous>(
    node: &mut ConnectedNetworkNode<T>,
    args: &Args,
) {
    let group = node.group_all();

    node.run(&CentralizedSync::new(), &group).unwrap().unwrap();

    let start = Instant::now();

    for _ in 0..args.batch_size {
        match args.algorithm {
            Centralized => node.run(&CentralizedSync::new(), &group),
            BinaryTree => node.run(&BinaryTreeSync::new(), &group),
            Dissemination => node.run(&DisseminationSync::new(), &group),
        }
        .unwrap()
        .unwrap();
    }

    let elapsed = start.elapsed();
    let nanos_for_barrier = elapsed.as_nanos() / args.batch_size as u128;
    println!(
        "[{}] -> Barrier in {} ns",
        node.rank_id(),
        nanos_for_barrier
    );
}

#[derive(Debug, Copy, Clone, clap::ValueEnum)]
enum BarrierAlgorithm {
    Centralized,
    BinaryTree,
    Dissemination,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config_file: String,
    #[arg(short, long)]
    rank_id: usize,
    #[arg(short, long)]
    num_nodes: usize,
    #[arg(short, long)]
    batch_size: usize,
    #[arg(short, long)]
    iters: Option<usize>,
    #[arg(short, long)]
    algorithm: BarrierAlgorithm,
}

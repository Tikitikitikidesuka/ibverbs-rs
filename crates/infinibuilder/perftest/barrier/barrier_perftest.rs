use BarrierAlgorithm::*;
use clap::Parser;
use infinibuilder::barrier::all_enum::{AnyBarrier, AnyBarrierType};
use infinibuilder::ibverbs::init::create_ibv_network_node;
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_network_node::RdmaNetworkNode;
use infinibuilder::transport::basic::BasicTransport;
use std::fs;
use std::time::Duration;
use tokio::time::Instant;

fn main() {
    simple_logger::init().unwrap();

    let args = Args::parse();
    let json_network = fs::read_to_string(&args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(args.num_nodes);

    let mut node = create_ibv_network_node(
        args.rank_id,
        32,
        512,
        network_config,
        Vec::new(),
        AnyBarrier::new(args.algorithm.into()),
        BasicTransport::new(),
    )
    .unwrap();

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

fn barrier_batch(node: &mut impl RdmaNetworkNode, args: &Args) {
    let group = node.group_all();
    node.barrier(&group, Duration::from_millis(200)).unwrap();

    let start = Instant::now();

    for i in 0..args.batch_size {
        //println!("Sync {} starting", i + 1);
        node.barrier(&group, Duration::from_millis(200)).unwrap()
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

impl Into<AnyBarrierType> for BarrierAlgorithm {
    fn into(self) -> AnyBarrierType {
        match self {
            Centralized => AnyBarrierType::Centralized,
            BinaryTree => AnyBarrierType::BinaryTree,
            Dissemination => AnyBarrierType::Dissemination,
        }
    }
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

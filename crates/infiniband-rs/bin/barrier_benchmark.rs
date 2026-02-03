use clap::{Parser, ValueEnum};
use infiniband_rs::ibverbs::open_device;
use infiniband_rs::network::Node;
use infiniband_rs::network::barrier::BarrierAlgorithm;
use infiniband_rs::network::config::RawNetworkConfig;
use infiniband_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::fs;
use std::time::{Duration, Instant};

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let args = Args::parse();

    let json_network = fs::read_to_string(&args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .truncate(args.world_size)
        .build()
        .unwrap();

    let node_config = network_config.get(args.rank).unwrap();

    let ctx = open_device(&node_config.ibdev).unwrap();
    let pd = ctx.allocate_pd().unwrap();

    let node = Node::builder()
        .pd(&pd)
        .rank(args.rank)
        .world_size(args.world_size)
        .barrier(args.algorithm.into())
        .build()
        .unwrap();

    let endpoint = node.endpoint();

    let remote_endpoints = Exchanger::await_exchange_all(
        args.rank,
        &network_config,
        &endpoint,
        &ExchangeConfig::default(),
    )
    .unwrap();

    let remote_endpoints = node.gather_endpoints(remote_endpoints).unwrap();

    let mut node = node.handshake(remote_endpoints).unwrap();

    if let Some(iters) = args.benchmark_iters {
        for _ in 0..iters {
            benchmark(&mut node, &args);
        }
    } else {
        loop {
            benchmark(&mut node, &args);
        }
    }
}

fn benchmark(node: &mut Node, args: &Args) {
    let start = Instant::now();

    let peers = (0..node.world_size()).collect::<Vec<_>>();

    for i in 0..args.sample_iters {
        node.barrier(&peers, Duration::from_millis(2000)).unwrap()
    }

    let elapsed = start.elapsed();
    let nanos_for_barrier = elapsed.as_nanos() / args.sample_iters as u128;
    println!("[{}] -> Barrier in {} ns", node.rank(), nanos_for_barrier);
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliBarrier {
    Centralized,
    BinaryTree,
    Dissemination,
}

impl From<CliBarrier> for BarrierAlgorithm {
    fn from(c: CliBarrier) -> Self {
        match c {
            CliBarrier::Centralized => BarrierAlgorithm::Centralized,
            CliBarrier::BinaryTree => BarrierAlgorithm::BinaryTree,
            CliBarrier::Dissemination => BarrierAlgorithm::Dissemination,
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config_file: String,
    #[arg(short, long)]
    rank: usize,
    #[arg(short, long)]
    world_size: usize,
    #[arg(short, long)]
    sample_iters: usize,
    #[arg(short, long)]
    benchmark_iters: Option<usize>,
    #[arg(short, long)]
    algorithm: CliBarrier,
}

use clap::{Parser, ValueEnum};
use infiniband_rs::ibverbs::open_device;
use infiniband_rs::network::Node;
use infiniband_rs::network::barrier::BarrierAlgorithm;
use infiniband_rs::network::config::RawNetworkConfig;
use infiniband_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use rand::Rng;
use std::fs;
use std::time::{Duration, Instant};

fn main() {
    let args = Args::parse();

    let rank = args.rank;
    let algorithm: BarrierAlgorithm = args.algorithm.into();
    let algorithm_str = format!("{:?}", args.algorithm).to_lowercase();
    let world_size = args.world_size;
    let sample_iters = args.sample_iters;

    println!(
        "[{rank}] -> barrier_benchmark[algorithm={algorithm_str},world_size={world_size},sample_iters={sample_iters}]",
    );

    let json_network = fs::read_to_string(&args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .truncate(world_size)
        .build()
        .unwrap();

    let node_config = network_config.get(rank).unwrap();

    let ctx = open_device(&node_config.ibdev).unwrap();
    let pd = ctx.allocate_pd().unwrap();

    let node = Node::builder()
        .pd(&pd)
        .rank(rank)
        .world_size(world_size)
        .barrier(algorithm)
        .build()
        .unwrap();

    let endpoint = node.endpoint();

    let remote_endpoints =
        Exchanger::await_exchange_all(rank, &network_config, &endpoint, &ExchangeConfig::default())
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
    let peers = (0..node.world_size()).collect::<Vec<_>>();
    let mut latencies = Vec::with_capacity(args.sample_iters);
    let mut rng = rand::rng();

    // 1. Warmup: Run unmeasured iterations to warm up instruction cache / branch predictors
    // and resolve any lazy network state.
    for _ in 0..100 {
        node.barrier_unchecked(&peers, Duration::from_millis(10000))
            .unwrap();
    }

    // 2. Coordinated Start: Ensure all nodes are ready to start the timer together.
    node.barrier_unchecked(&peers, Duration::from_secs(5))
        .unwrap();

    let start = Instant::now();

    for _ in 0..args.sample_iters {
        // 3. Jitter Injection: Simulate computation imbalance.
        // Without this, nodes naturally synchronize into a lockstep "wave" that
        // artificially inflates performance.
        if args.jitter_ns > 0 {
            let delay: u64 = rng.random_range(0..args.jitter_ns);
            if delay > 0 {
                std::thread::sleep(Duration::from_nanos(delay));
            }
        }

        let iter_start = Instant::now();
        node.barrier(&peers, Duration::from_millis(1000)).unwrap();
        latencies.push(iter_start.elapsed());
    }

    let elapsed = start.elapsed();
    let nanos_for_barrier = elapsed.as_nanos() / args.sample_iters as u128;
    println!("[{}] -> {} ns", args.rank, nanos_for_barrier);
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
    #[arg(long, default_value_t = 1024)]
    jitter_ns: u64,
    #[arg(short, long)]
    sample_iters: usize,
    #[arg(short, long)]
    benchmark_iters: Option<usize>,
    #[arg(short, long)]
    algorithm: CliBarrier,
}

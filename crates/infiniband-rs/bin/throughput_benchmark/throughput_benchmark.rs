use clap::Parser;
use infiniband_rs::ibverbs::open_device;
use infiniband_rs::multi_channel::work_request::{PeerReceiveWorkRequest, PeerSendWorkRequest};
use infiniband_rs::network::Node;
use infiniband_rs::network::config::RawNetworkConfig;
use infiniband_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use std::collections::VecDeque;
use std::fs;
use std::ptr::slice_from_raw_parts_mut;
use std::time::Duration;
use tokio::time::Instant;

fn main() {
    let args = Args::parse();

    let rank = args.rank;
    let max_parallel = args.max_parallel;
    println!("[{rank}] -> throughput_benchmark[max_parallel={max_parallel}]",);

    let json_network = fs::read_to_string(&args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .truncate(2)
        .build()
        .unwrap();

    let node_config = network_config.get(rank).unwrap();

    let ctx = open_device(&node_config.ibdev).unwrap();
    let pd = ctx.allocate_pd().unwrap();

    let node = Node::builder()
        .pd(&pd)
        .rank(rank)
        .world_size(2)
        .max_send_wr(args.max_parallel as u32)
        .max_recv_wr(args.max_parallel as u32)
        .build()
        .unwrap();

    let endpoint = node.endpoint();

    let remote_endpoints =
        Exchanger::await_exchange_all(rank, &network_config, &endpoint, &ExchangeConfig::default())
            .unwrap();

    let remote_endpoints = node.gather_endpoints(remote_endpoints).unwrap();

    let mut node = node.handshake(remote_endpoints).unwrap();

    if node.rank() == 0 {
        sender_benchmark(&mut node, &args);
    } else {
        receiver_benchmark(&mut node, &args);
    }
}

fn sender_benchmark(node: &mut Node, args: &Args) {
    let memory = vec![0u8; args.message_size].into_boxed_slice();
    let mr = node.pd().register_local_mr_slice(&memory).unwrap();

    let mut gbps_samples = Vec::with_capacity(args.sample_iters);
    let mut pps_samples = Vec::with_capacity(args.sample_iters);

    for iter in 0..(args.warmup_iters + args.sample_iters) {
        node.barrier(&[0, 1], Duration::from_secs(1)).unwrap();

        let start = Instant::now();

        node.scope(|s| {
            let mut sent = 0;
            let mut ongoing = VecDeque::with_capacity(args.max_parallel);
            while !ongoing.is_empty() || sent < args.message_count {
                if ongoing.len() < args.max_parallel {
                    ongoing.push_back(
                        s.post_send(PeerSendWorkRequest::new(
                            1,
                            &[mr.gather_element_unchecked(&memory)],
                        ))
                        .unwrap(),
                    );
                    sent += 1;
                } else {
                    ongoing.pop_front().unwrap().spin_poll().unwrap();
                }
            }
            Ok(())
        })
        .unwrap();

        let elapsed = start.elapsed().as_secs_f64();
        let bytes = (args.message_size * args.message_count) as f64;

        let gbps = (bytes * 8.0) / elapsed / 1e9;
        let pps = (args.message_count as f64) / elapsed;

        // Skip warmups.
        if iter >= args.warmup_iters {
            gbps_samples.push(gbps);
            pps_samples.push(pps);
        }

        let (mean_gbps, std_gbps) = mean_std(&gbps_samples);
        let (mean_pps, std_pps) = mean_std(&pps_samples);

        println!(
            "[{}] -> mean_gbps: {:.2} gbps, std_gbps: {:.2} gbps, mean_pps: {:.2} pps, std_pps: {:.2} pps",
            args.rank, mean_gbps, std_gbps, mean_pps, std_pps
        );
    }
}

fn receiver_benchmark(node: &mut Node, args: &Args) {
    let mut memory = vec![0u8; args.message_size * args.max_parallel].into_boxed_slice();
    let mr = node.pd().register_local_mr_slice(&memory).unwrap();

    let mut gbps_samples = Vec::with_capacity(args.sample_iters);
    let mut pps_samples = Vec::with_capacity(args.sample_iters);

    for iter in 0..(args.warmup_iters + args.sample_iters) {
        node.barrier(&[0, 1], Duration::from_secs(1)).unwrap();

        let start = Instant::now();

        node.scope(|s| {
            let mut received = 0;
            let mut ongoing = VecDeque::with_capacity(args.max_parallel);

            while !ongoing.is_empty() || received < args.message_count {
                if ongoing.len() < args.max_parallel {
                    ongoing.push_back(
                        s.post_receive(PeerReceiveWorkRequest::new(
                            1,
                            &mut [mr.scatter_element_unchecked(unsafe {
                                &mut *slice_from_raw_parts_mut(
                                    memory
                                        .as_mut_ptr()
                                        .add(args.message_size * (received % args.max_parallel)),
                                    args.message_size,
                                )
                            })],
                        ))
                        .unwrap(),
                    );
                    received += 1;
                } else {
                    ongoing.pop_front().unwrap().spin_poll().unwrap();
                }
            }
            Ok(())
        })
        .unwrap();

        let elapsed = start.elapsed().as_secs_f64();
        let bytes = (args.message_size * args.message_count) as f64;

        let gbps = (bytes * 8.0) / elapsed / 1e9;
        let pps = (args.message_count as f64) / elapsed;

        // Skip warmups.
        if iter >= args.warmup_iters {
            gbps_samples.push(gbps);
            pps_samples.push(pps);
        }

        let (mean_gbps, std_gbps) = mean_std(&gbps_samples);
        let (mean_pps, std_pps) = mean_std(&pps_samples);

        println!(
            "[{}] -> mean_gbps: {:.2} gbps, std_gbps: {:.2} gbps, mean_pps: {:.2} pps, std_pps: {:.2} pps",
            args.rank, mean_gbps, std_gbps, mean_pps, std_pps
        );
    }
}

fn mean_std(xs: &[f64]) -> (f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;
    (mean, var.sqrt())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config_file: String,
    #[arg(short, long)]
    rank: usize,
    #[arg(short, long)]
    message_size: usize,
    #[arg(short, long)]
    message_count: usize,
    #[arg(short, long)]
    max_parallel: usize,
    #[arg(short, long)]
    sample_iters: usize,
    #[arg(short, long)]
    warmup_iters: usize,
    #[arg(short, long)]
    benchmark_iters: Option<usize>,
}

use clap::Parser;
use infinibuilder::barrier::centralized::CentralizedBarrier;
use infinibuilder::ibverbs::init::create_ibv_network_node;
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_connection::RdmaWorkRequest;
use infinibuilder::rdma_network_node::{
    RdmaBarrierNetworkNode, RdmaGroupNetworkNode, RdmaNamedMemory,
    RdmaNamedMemoryRegionNetworkNode, RdmaNetworkNode, RdmaReceiveParams, RdmaSendParams,
};
use infinibuilder::transport::synced::SyncedTransport;
use std::fs;
use std::process::exit;
use std::time::{Duration, Instant};

fn main() {
    let args = Args::parse();
    let json_network = fs::read_to_string(&args.network_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(2);

    // Validate arguments
    if args.post_batch == 0 {
        eprintln!("Batch post count must be greater than zero");
        exit(1);
    }
    if args.message_count % args.post_batch != 0 {
        eprintln!("Message count must be a multiple of the post batch size");
        exit(1);
    }
    if args.max_parallel == 0 {
        eprintln!("Max parallel messages must be greater than zero");
        exit(1);
    }
    if args.max_parallel % args.post_batch != 0 {
        eprintln!("Max parallel messages must be a multiple of the post batch size");
        exit(1);
    }
    if args.rank_id != 0 && args.rank_id != 1 {
        eprintln!(
            "Rank id {} is not valid\nRank id must be 0 for sender node or 1 for receiver node",
            args.rank_id
        );
        exit(1);
    }

    let mem_name = "transfer";
    let memory_size = args.max_parallel * args.message_size;
    let mut memory = vec![0u8; memory_size];

    let mut node = create_ibv_network_node(
        args.rank_id,
        32,
        512,
        network_config,
        vec![RdmaNamedMemory::new(
            mem_name,
            memory.as_mut_ptr(),
            memory.len(),
        )],
        CentralizedBarrier::new(),
        SyncedTransport::with_post_timeout(Duration::from_millis(1000)),
    )
        .unwrap();

    let local_mr = node.local_mr(mem_name).unwrap();

    for iter_idx in 0..args.iters {
        // Sync all agents
        node.barrier(&node.group_all(), Duration::from_millis(1000))
            .unwrap();

        if args.rank_id == 0 {
            let throughput_sample = run_sender(&mut node, &local_mr, &args);
            println!(
                "Sender(iter={})[gbps={:.2},pps={:.2}]",
                iter_idx, throughput_sample.gbps, throughput_sample.pps
            );
        } else if args.rank_id == 1 {
            let throughput_sample = run_receiver(&mut node, &local_mr, &args);
            println!(
                "Receiver(iter={})[gbps={:.2},pps={:.2}]",
                iter_idx, throughput_sample.gbps, throughput_sample.pps
            );
        }

        // Sync all agents
        node.barrier(&node.group_all(), Duration::from_millis(1000))
            .unwrap();
    }
}

fn run_sender<MemoryRegion, Node: RdmaNetworkNode<MemoryRegion = MemoryRegion>>(
    node: &mut Node,
    memory_region: &MemoryRegion,
    args: &Args,
) -> ThroughputSample {
    let num_posts = args.message_count / args.post_batch;
    let num_send_configs = args.max_parallel / args.post_batch;

    let send_configs = (0..num_send_configs)
        .map(|send_type_idx| {
            (0..args.post_batch)
                .map(|send_idx| {
                    let start = send_type_idx * args.post_batch * args.message_size
                        + send_idx * args.message_size;
                    let end = start + args.message_size;
                    RdmaSendParams {
                        memory_region,
                        memory_range: start..end,
                        immediate_data: None,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut batched_wrs: Vec<Vec<_>> = (0..num_send_configs).map(|_| vec![]).collect();

    let start = Instant::now();

    let mut issued_sends = 0;
    let mut finished_sends = 0;

    while finished_sends < num_posts {
        // If max parallel sends allows more sends, send
        if issued_sends < num_posts && (issued_sends - finished_sends) < num_send_configs {
            let slot_idx = issued_sends % num_send_configs;
            // BUG WAS HERE: was using issued_sends / num_send_configs instead of %
            let wrs = node.post_send_batch(1, send_configs[slot_idx].as_slice());
            batched_wrs[slot_idx] = wrs;
            issued_sends += 1;
        } else {
            // Otherwise, wait
            let slot_idx = finished_sends % num_send_configs;
            let wrs = std::mem::replace(&mut batched_wrs[slot_idx], vec![]);
            for wr in wrs {
                let mut wr = wr.unwrap();
                wr.spin_poll_batched(Duration::from_millis(5000), 1024)
                    .unwrap();
            }
            finished_sends += 1;
        }
    }

    let elapsed = start.elapsed();
    let total_bytes = (args.message_size * args.message_count) as f64;
    let total_seconds = elapsed.as_secs_f64();
    let throughput_gbps = (total_bytes * 8.0) / (total_seconds * 1e9);
    let throughput_pps = args.message_count as f64 / total_seconds;

    ThroughputSample {
        gbps: throughput_gbps,
        pps: throughput_pps,
    }
}

fn run_receiver<MemoryRegion, Node: RdmaNetworkNode<MemoryRegion = MemoryRegion>>(
    node: &mut Node,
    memory_region: &MemoryRegion,
    args: &Args,
) -> ThroughputSample {
    let num_posts = args.message_count / args.post_batch;
    let num_recv_configs = args.max_parallel / args.post_batch;

    let recv_configs = (0..num_recv_configs)
        .map(|recv_type_idx| {
            (0..args.post_batch)
                .map(|recv_idx| {
                    let start = recv_type_idx * args.post_batch * args.message_size
                        + recv_idx * args.message_size;
                    let end = start + args.message_size;
                    RdmaReceiveParams {
                        memory_region,
                        memory_range: start..end,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut batched_wrs: Vec<Vec<_>> = (0..num_recv_configs).map(|_| vec![]).collect();

    let start = Instant::now();

    let mut issued_recvs = 0;
    let mut finished_recvs = 0;

    while finished_recvs < num_posts {
        // Post receives if we have room in the pipeline
        if issued_recvs < num_posts && (issued_recvs - finished_recvs) < num_recv_configs {
            let slot_idx = issued_recvs % num_recv_configs;
            let wrs = node.post_receive_batch(0, recv_configs[slot_idx].as_slice());
            batched_wrs[slot_idx] = wrs;
            issued_recvs += 1;
        } else {
            // Wait for completion
            let slot_idx = finished_recvs % num_recv_configs;
            let wrs = std::mem::replace(&mut batched_wrs[slot_idx], vec![]);
            for wr in wrs {
                let mut wr = wr.unwrap();
                wr.spin_poll_batched(Duration::from_millis(5000), 1024)
                    .unwrap();
            }
            finished_recvs += 1;
        }
    }

    let elapsed = start.elapsed();
    let total_bytes = (args.message_size * args.message_count) as f64;
    let total_seconds = elapsed.as_secs_f64();
    let throughput_gbps = (total_bytes * 8.0) / (total_seconds * 1e9);
    let throughput_pps = args.message_count as f64 / total_seconds;

    ThroughputSample {
        gbps: throughput_gbps,
        pps: throughput_pps,
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    message_size: usize,
    #[arg(short, long)]
    message_count: usize,
    #[arg(short, long)]
    max_parallel: usize,
    #[arg(short, long)]
    post_batch: usize,
    #[arg(short, long)]
    iters: usize,
    #[arg(short, long)]
    rank_id: usize,
    #[arg(short, long)]
    network_file: String,
}

struct ThroughputSample {
    gbps: f64,
    pps: f64,
}

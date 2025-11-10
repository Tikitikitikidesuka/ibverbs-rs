use clap::Parser;
use infinibuilder::barrier::centralized::CentralizedBarrier;
use infinibuilder::ibverbs::init::create_ibv_network_node;
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_connection::RdmaWorkRequest;
use infinibuilder::rdma_network_node::{
    RdmaNamedMemory, RdmaNamedMemoryRegionNetworkNode, RdmaNetworkNode,
};
use infinibuilder::transport::synced::SyncedTransport;
use std::fs;
use std::ptr::slice_from_raw_parts;
use std::time::{Duration, Instant};

/// This benchmark's objective is finding the throughput between two nodes, a sender and a receiver.
/// A memory region with size `batch number of messages * message size`. During the execution,
/// the sender node will send contiguous slices of size `message size` to the receiver node.
/// The receiver will receive them in its memory region to their corresponding locations.
///
/// REMOVED CORRECTNESS CHECK
/// To check the correctness of the communication, the sender will fill its buffer with ascending numbers
/// from zero in the body of unsigned eight bit integers (wrap at 256); the receiver, in turn, will
/// check that the final values in its memory follow the same pattern.
///
/// The execution of the batch of sends will be repeated `iters` times before concluding the benchmark.
/// If `iters` is `None` then the benchmark will run until killed.

fn main() {
    simple_logger::init().unwrap();

    let args = Args::parse();
    let json_network = fs::read_to_string(&args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(args.num_nodes);

    if args.pipeline_size > args.batch_size {
        panic!("Pipeline size cannot be larger than batch size")
    }

    let mem_name = "foo";
    let memory_size = args.message_size * args.pipeline_size;
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

    let iters = args.iters.unwrap_or(usize::MAX);
    for iter in 0..iters {
        match args.rank_id {
            0 => {
                let local_mr = node.local_mr(mem_name).unwrap();
                sender_batch(
                    iter,
                    memory.as_mut_ptr(),
                    memory.len(),
                    &local_mr,
                    &mut node,
                    &args,
                )
            }
            1 => {
                let local_mr = node.local_mr(mem_name).unwrap();
                receiver_batch(
                    iter,
                    memory.as_mut_ptr(),
                    memory.len(),
                    &local_mr,
                    &mut node,
                    &args,
                )
            }
            id => unreachable!("This node ({id}) does not participate in the benchmark"),
        }
    }
}

fn sender_batch<NetworkNode: RdmaNetworkNode>(
    iter: usize,
    mem_address: *mut u8,
    mem_length: usize,
    mr: &NetworkNode::MemoryRegion,
    node: &mut NetworkNode,
    args: &Args,
) {
    // Initialize memory for correctness check later on
    /*
    (0..mem_length)
        .for_each(|i| unsafe { mem_address.add(i).write_volatile(((i + iter) % 256) as u8) });

     */

    let mut wr_queue = Vec::with_capacity(args.pipeline_size);

    // Wait until receiver is ready
    println!("Initial rendezvous...");
    node.barrier(&node.group_all(), Duration::from_millis(10000))
        .unwrap();
    println!("Synchronized");

    // Start timing
    let start = Instant::now();

    // Initial pipeline fill
    for i in 0..args.pipeline_size {
        wr_queue.push(
            node.post_send(
                1,
                mr,
                ((i % args.pipeline_size) * args.message_size)
                    ..(((i % args.pipeline_size) + 1) * args.message_size),
                None,
            )
            .unwrap(),
        )
    }

    // Send all batches
    for i in args.pipeline_size..args.batch_size {
        // Wait until one is finished
        wr_queue[i % args.pipeline_size]
            .spin_poll_batched(Duration::from_millis(5000), 1024)
            .unwrap();

        // Feed another to the pipeline
        let wr = node
            .post_send(
                1,
                mr,
                ((i % args.pipeline_size) * args.message_size)
                    ..(((i % args.pipeline_size) + 1) * args.message_size),
                None,
            )
            .unwrap();

        wr_queue[i % args.pipeline_size] = wr;
    }

    for i in args.batch_size..(args.batch_size + args.pipeline_size) {
        wr_queue[i % args.pipeline_size]
            .spin_poll_batched(Duration::from_millis(5000), 1024)
            .unwrap();
    }

    // Finish timing
    let elapsed = start.elapsed();

    println!("Finished");

    // Print results
    let pps = args.batch_size as f64 / elapsed.as_secs_f64();
    let gbps =
        (args.batch_size * args.message_size * 8) as f64 / (1000000000_f64 * elapsed.as_secs_f64());
    println!("pps: {pps:.2}, gbps: {gbps:.2}");
}

fn receiver_batch<NetworkNode: RdmaNetworkNode>(
    iter: usize,
    mem_address: *mut u8,
    mem_length: usize,
    mr: &NetworkNode::MemoryRegion,
    node: &mut NetworkNode,
    args: &Args,
) {
    let mut wr_queue = Vec::with_capacity(args.pipeline_size);

    // Notify sender to start
    println!("Initial rendezvous...");
    node.barrier(&node.group_all(), Duration::from_millis(10000))
        .unwrap();
    println!("Synchronized");

    // Initial pipeline fill
    for i in 0..args.pipeline_size {
        wr_queue.push(
            node.post_receive(
                0,
                mr,
                ((i % args.pipeline_size) * args.message_size)
                    ..(((i % args.pipeline_size) + 1) * args.message_size),
            )
            .unwrap(),
        )
    }

    // Send all batches
    for i in args.pipeline_size..args.batch_size {
        // Wait until one is finished
        wr_queue[i % args.pipeline_size]
            .spin_poll_batched(Duration::from_millis(5000), 1024)
            .unwrap();

        // Feed another to the pipeline
        let wr = node
            .post_receive(
                0,
                mr,
                ((i % args.pipeline_size) * args.message_size)
                    ..(((i % args.pipeline_size) + 1) * args.message_size),
            )
            .unwrap();

        wr_queue[i % args.pipeline_size] = wr;
    }

    for i in args.batch_size..(args.batch_size + args.pipeline_size) {
        wr_queue[i % args.pipeline_size]
            .spin_poll_batched(Duration::from_millis(5000), 1024)
            .unwrap();
    }

    // Validate transfer
    /*
    let memory = unsafe { &*slice_from_raw_parts(mem_address, mem_length) };
    println!("Memory: {:?}", &memory[..10]);
    if !memory
        .iter()
        .enumerate()
        .all(|(i, v)| *v == ((i + iter) % 256) as u8)
    {
        panic!("Memory not transferred correctly")
    } else {
        println!("Memory transferred correctly");
    }
    */
}

#[derive(Debug, Copy, Clone, clap::ValueEnum)]
enum Role {
    Sender,
    Receiver,
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
    message_size: usize,
    #[arg(short, long)]
    batch_size: usize,
    #[arg(short, long)]
    pipeline_size: usize,
    #[arg(short, long)]
    iters: Option<usize>,
}

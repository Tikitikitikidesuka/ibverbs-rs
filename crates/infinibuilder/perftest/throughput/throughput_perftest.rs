use clap::Parser;
use infinibuilder::connect::Connect;
use infinibuilder::network::{ConnectedNetworkNode, NetworkNodeConnectionConfig};
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_traits::WorkRequest;
use infinibuilder::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use infinibuilder::tcp_exchanger::{TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig};
use std::fs;
use std::time::{Duration, Instant};

/// This benchmark's objective is finding the throughput between two nodes, a sender and a receiver.
/// A memory region with size `batch number of messages * message size`. During the execution,
/// the sender node will send contiguous slices of size `message size` to the receiver node.
/// The receiver will receive them in its memory region to their corresponding locations.
///
/// To check the correctness of the communication, the sender will fill its buffer with ascending numbers
/// from zero in the body of unsigned eight bit integers (wrap at 256); the receiver, in turn, will
/// check that the final values in its memory follow the same pattern.
///
/// The execution of the batch of sends will be repeated `iters` times before concluding the benchmark.
/// If `iters` is `None` then the benchmark will run until killed.

fn main() {
    let args = Args::parse();
    let json_network = fs::read_to_string(&args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(args.num_nodes)
        .validate()
        .unwrap();

    let memory_size = args.message_size * args.batch_size;
    let mut memory = vec![0; memory_size];
    let rank_id = match args.role {
        Role::Sender => 0,
        Role::Receiver => 1,
    };

    let node = unsafe {
        ConnectedNetworkNode::new_ibv_simple_unit_network_node::<64, 64>(
            rank_id,
            &network_config,
            memory.as_mut_ptr(),
            memory.len(),
        )
    }
    .unwrap();

    let out_conn_config = node.connection_config();

    let exchanger_network = TcpExchangerNetworkConfig::from_network(network_config).unwrap();
    let exchanged = TcpExchanger::await_exchange_network_config(
        rank_id,
        &out_conn_config,
        &exchanger_network,
        &exchanger_config(),
    )
    .unwrap();

    let in_conn_config =
        NetworkNodeConnectionConfig::gather(rank_id, exchanged.as_slice()).unwrap();

    let mut node = node.connect(in_conn_config).unwrap();

    let mut task: Box<dyn FnMut(usize)> = match args.role {
        Role::Sender => Box::new(|iter: usize| {
            master_batch(iter, &mut memory, node.connection(1).unwrap(), &args);
        }),
        Role::Receiver => Box::new(|iter: usize| {
            slave_batch(iter, &memory, node.connection(0).unwrap(), &args);
        }),
    };

    let mut iter = 0;
    while args.iters.map_or(true, |max| iter < max) {
        task(iter);
        iter += 1;
    }
}

fn master_batch(
    iter: usize,
    memory: &mut [u8],
    conn: &mut (impl RdmaSendRecv + RdmaRendezvous),
    args: &Args,
) {
    // Initialize memory for correctness check
    memory
        .iter_mut()
        .enumerate()
        .for_each(|(i, v)| *v = ((i + iter) % 256) as u8);

    // Wait until receiver is ready to receive
    println!("Initial rendezvous...");
    conn.rendezvous().unwrap();

    // Start timing
    let start = Instant::now();

    // Send all batches
    for i in 0..args.batch_size {
        println!("Rendezvous...");
        conn.rendezvous().unwrap();
        println!("Sending...");
        unsafe { conn.post_send((i * args.message_size)..((i + 1) * args.message_size), None) }
            .unwrap()
            .wait()
            .unwrap();
    }

    // Wait until receiver finishes receiving
    println!("Final rendezvous...");
    conn.rendezvous().unwrap();

    // Stop timing
    let elapsed = start.elapsed();

    // Print results
    let pps = args.batch_size as f64 / elapsed.as_secs_f64();
    let gbps =
        (args.batch_size * args.message_size * 8) as f64 / (1000000000_f64 * elapsed.as_secs_f64());
    println!("pps: {pps:.2}, gbps: {gbps:.2}");
}

fn slave_batch(
    iter: usize,
    memory: &[u8],
    conn: &mut (impl RdmaSendRecv + RdmaRendezvous),
    args: &Args,
) {
    // Notify sender to start
    println!("Initial rendezvous...");
    conn.rendezvous().unwrap();

    // Receive all batches
    for i in 0..args.batch_size {
        println!("Receiving...");
        let wr =
            unsafe { conn.post_receive((i * args.message_size)..((i + 1) * args.message_size)) }
                .unwrap();
        println!("Rendezvous...");
        conn.rendezvous().unwrap();
        wr.wait().unwrap();
    }

    // Notify sender of finish
    println!("Final rendezvous...");
    conn.rendezvous().unwrap();

    // Validate transfer
    println!("Memory: {:?}", &memory[..10]);
    if !memory
        .iter()
        .enumerate()
        .all(|(i, v)| *v == ((i + iter) % 256) as u8)
    {
        panic!("Memory not transferred correctly")
    } else {
        println!("Memory transfered corretly");
    }
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
    role: Role,
    #[arg(short, long)]
    num_nodes: usize,
    #[arg(short, long)]
    message_size: usize,
    #[arg(short, long)]
    batch_size: usize,
    #[arg(short, long)]
    iters: Option<usize>,
}

fn exchanger_config() -> TcpExchangerConfig {
    TcpExchangerConfig {
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    }
}

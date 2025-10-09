use clap::Parser;
use infinibuilder::connect::Connect;
use infinibuilder::network::{ConnectedNetworkNode, NetworkNodeConnectionConfig};
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_traits::{RdmaSync, RdmaSendRecv};
use infinibuilder::tcp_exchanger::{TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig};
use std::fs;
use std::time::Duration;
use tokio::time::Instant;

fn main() {
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

    let exchanger_network = TcpExchangerNetworkConfig::from_network(network_config).unwrap();
    let exchanged = TcpExchanger::await_exchange_network_config(
        args.rank_id,
        &out_conn_config,
        &exchanger_network,
        &exchanger_config(),
    )
    .unwrap();

    let in_conn_config =
        NetworkNodeConnectionConfig::gather(args.rank_id, exchanged.as_slice()).unwrap();

    let mut node = node.connect(in_conn_config).unwrap();

    match args.rank_id {
        0 => {
            if let Some(iters) = args.iters {
                for _ in 0..iters {
                    rendezvous_batch(node.connection(1).unwrap(), &args);
                }
            } else {
                loop {
                    rendezvous_batch(node.connection(1).unwrap(), &args);
                }
            }
        }
        1 => {
            if let Some(iters) = args.iters {
                for _ in 0..iters {
                    rendezvous_batch(node.connection(0).unwrap(), &args);
                }
            } else {
                loop {
                    rendezvous_batch(node.connection(0).unwrap(), &args);
                }
            }
        }
        _ => panic!(
            "Rank id {} does not participate in this benchmark",
            args.rank_id
        ),
    }
}

fn rendezvous_batch(conn: &mut (impl RdmaSendRecv + RdmaSync), args: &Args) {
    let start = Instant::now();

    for _ in 0..args.batch_size {
        conn.rendezvous().unwrap();
    }

    let elapsed = start.elapsed();
    let nanos_for_rendezvous = elapsed.as_nanos() / args.batch_size as u128;
    println!("Rendezvous in {} ns", nanos_for_rendezvous);
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
}

fn exchanger_config() -> TcpExchangerConfig {
    TcpExchangerConfig {
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    }
}

use infinibuilder::connect::Connect;
use infinibuilder::network::{ConnectedNetworkNode, NetworkNodeConnectionConfig};
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_traits::WorkRequest;
use infinibuilder::rdma_traits::{RdmaSync, RdmaSendRecv};
use infinibuilder::tcp_exchanger::{TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig};
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

fn main() {
    let run_params = parse_args();
    let json_network = fs::read_to_string(run_params.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .validate()
        .unwrap();
    let memory = vec![0; run_params.message_size];

    let node = ConnectedNetworkNode::new_ibv_simple_unit_network_node::<64, 64>(
        run_params.rank_id,
        &network_config,
        &memory.as_slice(),
    )
    .unwrap();

    let out_conn_config = node.connection_config();

    let exchanger_network = TcpExchangerNetworkConfig::from_network(network_config).unwrap();
    let exchanged = TcpExchanger::await_exchange_network_config(
        run_params.rank_id,
        &out_conn_config,
        &exchanger_network,
        &exchanger_config(),
    )
    .unwrap();

    let in_conn_config =
        NetworkNodeConnectionConfig::gather(run_params.rank_id, exchanged.as_slice()).unwrap();

    let mut node = node.connect(in_conn_config).unwrap();

    const AVG_ITERS: usize = 5;
    const ITERS: usize = 1000;

    match run_params.rank_id {
        0 => {
            let start = Instant::now();

            for _ in 0..ITERS {
                node.connection(1).unwrap().rendezvous().unwrap();
                let wr = unsafe {
                    node.connection(1)
                        .unwrap()
                        .post_send(0..run_params.message_size, None)
                        .unwrap()
                };
                wr.wait().unwrap();
            }

            let time = start.elapsed();
            let sent_bytes = ITERS * run_params.message_size;

            let throughput_gbps =
                (sent_bytes as f64 * 8.0) / (time.as_secs_f64() * 10f64.powf(9.0));
            println!("Sender throughput: {throughput_gbps} Gbps");
        }
        1 => {
            for _ in 0..ITERS {
                let wr = unsafe {
                    node.connection(0)
                        .unwrap()
                        .post_receive(0..run_params.message_size)
                        .unwrap()
                };
                node.connection(0).unwrap().rendezvous().unwrap();
                wr.wait().unwrap();
            }
        }
        _ => {}
    }
}

struct Args {
    rank_id: usize,
    config_file: String,
    message_size: usize,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} config_file rank_id message_size", args[0]);
        std::process::exit(1);
    }

    let config_file = args[1].parse().unwrap();
    let rank_id = usize::from_str(args[2].as_str()).unwrap();
    let message_size = usize::from_str(args[3].as_str()).unwrap();

    Args {
        rank_id,
        config_file,
        message_size,
    }
}

fn exchanger_config() -> TcpExchangerConfig {
    TcpExchangerConfig {
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    }
}

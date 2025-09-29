use infinibuilder::connect::Connect;
use infinibuilder::network::{ConnectedNetworkNode, NetworkNodeConnectionConfig};
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::synchronization::centralized::CentralizedSync;
use infinibuilder::synchronization::dissemination::DisseminationSync;
use infinibuilder::tcp_exchanger::{TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig};
use rand::Rng;
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::{env, fs};
use infinibuilder::synchronization::binary::BinaryTreeSync;

fn main() {
    let run_params = parse_args();
    let json_network = fs::read_to_string(run_params.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .validate()
        .unwrap();
    let memory = [0; 1024];

    let node = ConnectedNetworkNode::new_ibv_simple_unit_network_node::<64, 64>(
        run_params.rank_id,
        &network_config,
        &memory,
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

    let mut rng = rand::rng();
    let delay_ms = rng.random_range(1000..=5000);
    std::thread::sleep(Duration::from_millis(delay_ms));
    println!("{} -> Sending first sync...", run_params.rank_id);
    node.run(&BinaryTreeSync::with_timeout(Duration::from_millis(5000)), &node.group_all())
        .unwrap()
        .unwrap();
    println!("{} -> First sync finished", run_params.rank_id);
    std::thread::sleep(Duration::from_millis(delay_ms));
    println!("{} -> Sending second sync...", run_params.rank_id);
    node.run(&BinaryTreeSync::with_timeout(Duration::from_millis(5000)), &node.group_all())
        .unwrap()
        .unwrap();
    println!("{} -> Second sync finished", run_params.rank_id);
}
struct Args {
    rank_id: usize,
    config_file: String,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} config_file rank_id", args[0]);
        std::process::exit(1);
    }

    let config_file = args[1].parse().unwrap();
    let rank_id = usize::from_str(args[2].as_str()).unwrap();

    Args {
        rank_id,
        config_file,
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

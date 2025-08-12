use ibverbs::QueuePairEndpoint;
use infinibuilder::{IbBDynamicNodeConfig, IbBStaticNodeConfig, IbBTcpNetworkConfigExchanger, IbBTcpNetworkConfigExchangerConfig, IbBUncheckedStaticNetworkConfig};
use std::env;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rank_id>", args[0]);
        std::process::exit(1);
    }

    let rank_id: u32 = args[1].parse().expect("Invalid rank ID");

    // Create the static network configuration
    let network_config = IbBUncheckedStaticNetworkConfig::new()
        .add_node(IbBStaticNodeConfig::new("tdeb01", "keo", 0, "keo"))
        .add_node(IbBStaticNodeConfig::new("tdeb02", "keo", 1, "keo"))
        .add_node(IbBStaticNodeConfig::new("tdeb03", "keo", 2, "keo"))
        .add_node(IbBStaticNodeConfig::new("tdeb04", "keo", 3, "keo"))
        .validate()
        .expect("Failed to validate network configuration");

    // Configure the TCP exchanger settings
    let exchanger_config = IbBTcpNetworkConfigExchangerConfig {
        tcp_port: 8844,
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    };

    let my_endpoint = my_qp_endpoint(rank_id);

    let dynamic_config = IbBDynamicNodeConfig {
        qp_endpoint: my_endpoint,
    };

    println!(
        "Starting network configuration exchange for rank {}...",
        rank_id
    );

    // Bind to all interfaces on the specified port
    let bind_addr = format!("0.0.0.0:{}", exchanger_config.tcp_port);

    // Exchange network configuration (send and receive simultaneously)
    match IbBTcpNetworkConfigExchanger::await_exchange_network_config(
        rank_id,
        &dynamic_config,
        bind_addr,
        &network_config,
        &exchanger_config,
    ) {
        Ok(ready_network) => {
            println!("✅ Successfully exchanged network configuration!");
            println!("Complete network:");
            ready_network.iter().for_each(|node| {
                println!("  Node {}: {:?}", node.rank_id(), node.dynamic_config().qp_endpoint);
            });
        }
        Err(e) => {
            eprintln!("❌ Failed to exchange network config: {}", e);
            std::process::exit(1);
        }
    }

    println!("Network configuration exchange completed!");
}

fn my_qp_endpoint(rank_id: u32) -> QueuePairEndpoint {
    match rank_id {
        0 => serde_json::from_str("{\"num\":12384,\"lid\":0,\"gid\":null}").unwrap(),
        1 => serde_json::from_str("{\"num\":12384,\"lid\":1,\"gid\":null}").unwrap(),
        2 => serde_json::from_str("{\"num\":12384,\"lid\":2,\"gid\":null}").unwrap(),
        3 => serde_json::from_str("{\"num\":12384,\"lid\":3,\"gid\":null}").unwrap(),
        _ => panic!("Invalid rank_id: {}", rank_id),
    }
}

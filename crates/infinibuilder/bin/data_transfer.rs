use infinibuilder::tcp_exchanger::{
    TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig, TcpExchangerNodeConfig,
};
use std::env;
use std::time::Duration;
use infinibuilder::transfer::common::{ConnectionInputConfig, TransferConfig, UnconnectedTransfer};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rank_id>", args[0]);
        std::process::exit(1);
    }

    let rank_id: u32 = args[1].parse().expect("Invalid rank ID");

    // Set up network with 3 nodes: 1 transmitter (0) and 2 receivers (1, 2)
    let exchanger_network_config = TcpExchangerNetworkConfig::new()
        .add_node(TcpExchangerNodeConfig::new(0, "tdeb01".to_string())).unwrap()
        .add_node(TcpExchangerNodeConfig::new(1, "tdeb02".to_string())).unwrap()
        .add_node(TcpExchangerNodeConfig::new(2, "tdeb03".to_string())).unwrap();

    exchanger_network_config
        .get(&rank_id)
        .expect("Rank id not in network");

    let role = match rank_id {
        0 => Role::Transmitter,
        1 | 2 => Role::Receiver,
        _ => panic!("Invalid rank ID, must be 0, 1, or 2"),
    };

    // Open RDMA device
    let devices = ibverbs::devices()?;
    println!("Devices: {:?}", devices.iter().map(|d| d.name()).collect::<Vec<_>>());
    let context = devices
        .get(0)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No RDMA devices found"))?
        .open()?;

    // Create memory buffer for data transfer
    let mut data_buffer = vec![0u8; 1024];
    if role == Role::Transmitter {
        // Fill buffer with test data
        for (i, byte) in data_buffer.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        println!("Transmitter: Initialized buffer with test data");
    }

    // Configure transfer based on role
    let num_peers = match role {
        Role::Transmitter => 2, // Connect to 2 receivers
        Role::Receiver => 1,    // Connect to 1 transmitter
    };

    let transfer_config = unsafe { TransferConfig::new(num_peers, &data_buffer) };

    // Create unconnected transfer
    let unconnected = UnconnectedTransfer::new(context, transfer_config)?;
    let local_conn_cfg = unconnected.connection_config();

    // Exchange connection configs
    let exchanger_config = TcpExchangerConfig {
        tcp_port: 8844,
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    };

    let exchanged_configs = TcpExchanger::await_exchange_network_config(
        rank_id,
        &local_conn_cfg,
        &exchanger_network_config,
        &exchanger_config,
    ).unwrap();

    println!("Exchanged configs: {:?}", exchanged_configs.as_slice());

    // Process exchanged configs based on role
    let peer_input_config = match role {
        Role::Transmitter => {
            // Transmitter needs configs from both receivers
            let receiver_configs: Vec<_> = exchanged_configs
                .iter()
                .filter(|node| node.node_id == 1 || node.node_id == 2)
                .map(|node| node.data.clone())
                .collect();

            if receiver_configs.len() != 2 {
                panic!("Expected 2 receiver configs, got {}", receiver_configs.len());
            }

            ConnectionInputConfig::gather_connection_config(receiver_configs, 0)
                .expect("Failed to gather receiver configs")
        }
        Role::Receiver => {
            // Receiver needs config from transmitter
            let transmitter_configs: Vec<_> = exchanged_configs
                .iter()
                .filter(|node| node.node_id == 0)
                .map(|node| node.data.clone())
                .collect();

            if transmitter_configs.len() != 1 {
                panic!("Expected 1 transmitter config, got {}", transmitter_configs.len());
            }

            // Remote index is the receiver's position in transmitter's peer list
            let remote_idx = match rank_id {
                1 => 0, // First receiver
                2 => 1, // Second receiver
                _ => unreachable!(),
            };

            ConnectionInputConfig::gather_connection_config(transmitter_configs, remote_idx)
                .expect("Failed to gather transmitter config")
        }
    };

    // Connect
    let mut connected = unconnected.connect(peer_input_config)?;
    println!("Connected.");

    // Perform data transfer based on role
    match role {
        Role::Transmitter => {
            println!("Transmitting data...");

            wait_key("Press a key to send to receiver 1...");
            println!("Sending first 512 bytes to receiver 1...");
            let wc = connected.wait_send(0, 0..512).unwrap();
            println!("Send to receiver 1 completed: status={:?}", wc.is_valid());

            wait_key("Press a key to send to receiver 2...");
            println!("Sending next 512 bytes to receiver 2...");
            let wc = connected.wait_send(1, 512..1024).unwrap();
            println!("Send to receiver 2 completed: status={:?}", wc.is_valid());

            println!("All transfers completed!");
        }
        Role::Receiver => {
            let receive_range = match rank_id {
                1 => {
                    println!("Receiver 1: Waiting to receive first 512 bytes...");
                    0..512
                }
                2 => {
                    println!("Receiver 2: Waiting to receive next 512 bytes...");
                    512..1024
                }
                _ => unreachable!(),
            };

            let wc = connected.wait_receive(0, receive_range.clone()).unwrap();
            println!("Receive completed: status={:?}", wc.is_valid());

            // Print some received data
            let start = receive_range.start;
            println!(
                "First 10 bytes received at offset {}: {:?}",
                start,
                &data_buffer[start..start + 10.min(receive_range.len())]
            );
        }
    }

    Ok(())
}

fn wait_key(message: &str) {
    println!("{}", message);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Role {
    Transmitter,
    Receiver,
}

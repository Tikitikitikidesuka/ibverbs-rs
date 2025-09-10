use infinibuilder::config_exchange::{
    TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig, TcpExchangerNodeConfig,
};
use infinibuilder::synchronization::SyncComponent;
use infinibuilder::synchronization::centralized::common::{
    CentralizedSyncConfig, CentralizedSyncConnectionInputConfig,
    CentralizedSyncConnectionOutputConfig, UnconnectedCentralizedSync,
};
use infinibuilder::synchronization::centralized::master::MasterConnectionInputConfig;
use infinibuilder::synchronization::centralized::slave::SlaveConnectionInputConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use infinibuilder::transfer::common::{ConnectionInputConfig, ConnectionOutputConfig, TransferConfig, UnconnectedTransfer};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeConfigs {
    transfer_config: ConnectionOutputConfig,
    sync_config: CentralizedSyncConnectionOutputConfig,
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rank_id>", args[0]);
        std::process::exit(1);
    }

    let rank_id: u32 = args[1].parse().expect("Invalid rank ID");

    // Set up network: ranks 0,1,2 = senders, ranks 3,4,5 = receivers
    let exchanger_network_config = TcpExchangerNetworkConfig::new()
        .add_node(TcpExchangerNodeConfig::new(0, "tdeb01".to_string())).unwrap()
        .add_node(TcpExchangerNodeConfig::new(1, "tdeb02".to_string())).unwrap()
        .add_node(TcpExchangerNodeConfig::new(2, "tdeb03".to_string())).unwrap()
        .add_node(TcpExchangerNodeConfig::new(3, "tdeb05".to_string())).unwrap()
        .add_node(TcpExchangerNodeConfig::new(4, "tdeb06".to_string())).unwrap()
        .add_node(TcpExchangerNodeConfig::new(5, "tdeb07".to_string())).unwrap();

    exchanger_network_config
        .get(&rank_id)
        .expect("Rank id not in network");

    let role = match rank_id {
        0..=2 => Role::Sender(rank_id),
        3..=5 => Role::Receiver(rank_id),
        _ => panic!("Invalid rank ID, must be 0-5"),
    };

    // Open RDMA device
    let devices = ibverbs::devices()?;
    println!("Devices: {:?}", devices.iter().map(|d| d.name()).collect::<Vec<_>>());
    let context = devices
        .get(0)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No RDMA devices found"))?
        .open()?;

    // Create memory buffer - 3 rounds × 256 bytes each = 768 bytes per sender
    let mut data_buffer = vec![0u8; 768];
    if let Role::Sender(sender_id) = role {
        // Each sender uses a unique pattern within u8 range
        for (i, byte) in data_buffer.iter_mut().enumerate() {
            *byte = ((sender_id as usize * 80 + i) % 256) as u8;
        }
        println!("Sender {}: Initialized buffer with unique pattern", sender_id);
    }

    // Configure transfer (each node connects to 3 peers)
    let transfer_config = unsafe { TransferConfig::new(3, &data_buffer) };
    let unconnected_transfer = UnconnectedTransfer::new(&context, transfer_config)?;
    let transfer_conn_cfg = unconnected_transfer.connection_config();

    // Configure sync (centralized with rank 0 as master)
    let sync_config = if rank_id == 0 {
        CentralizedSyncConfig::new_master(5) // 5 slaves
    } else {
        CentralizedSyncConfig::new_slave((rank_id - 1) as usize) // slaves indexed 0-4
    };
    let unconnected_sync = UnconnectedCentralizedSync::new(&context, sync_config)?;
    let sync_conn_cfg = unconnected_sync.connection_config();

    // Package both configs
    let local_configs = NodeConfigs {
        transfer_config: transfer_conn_cfg,
        sync_config: sync_conn_cfg,
    };

    // Exchange all configs
    let exchanger_config = TcpExchangerConfig {
        tcp_port: 8844,
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    };

    let exchanged_configs = TcpExchanger::await_exchange_network_config(
        rank_id,
        &local_configs,
        &exchanger_network_config,
        &exchanger_config,
    ).unwrap();

    println!("Exchanged configs from {} nodes", exchanged_configs.as_slice().len());

    // Set up transfer connections
    let transfer_peer_config = match role {
        Role::Sender(_) => {
            // Senders connect to receivers (ranks 3,4,5)
            let receiver_configs: Vec<_> = exchanged_configs
                .iter()
                .filter(|node| node.node_id >= 3 && node.node_id <= 5)
                .map(|node| node.data.transfer_config.clone())
                .collect();

            ConnectionInputConfig::gather_connection_config(receiver_configs, rank_id as usize)
                .expect("Failed to gather receiver configs")
        }
        Role::Receiver(_) => {
            // Receivers connect to senders (ranks 0,1,2)
            let sender_configs: Vec<_> = exchanged_configs
                .iter()
                .filter(|node| node.node_id <= 2)
                .map(|node| node.data.transfer_config.clone())
                .collect();

            let remote_idx = (rank_id - 3) as usize; // Map 3->0, 4->1, 5->2
            ConnectionInputConfig::gather_connection_config(sender_configs, remote_idx)
                .expect("Failed to gather sender configs")
        }
    };

    // Set up sync connections
    let sync_input_config = {
        let slave_configs: Vec<_> = exchanged_configs
            .iter()
            .filter_map(|node| {
                if let CentralizedSyncConnectionOutputConfig::Slave(slave) = &node.data.sync_config {
                    Some(slave.clone())
                } else {
                    None
                }
            })
            .collect();

        let master_config = exchanged_configs
            .iter()
            .find_map(|node| {
                if let CentralizedSyncConnectionOutputConfig::Master(master) = &node.data.sync_config {
                    Some(master.clone())
                } else {
                    None
                }
            })
            .expect("No master config found");

        if rank_id == 0 {
            CentralizedSyncConnectionInputConfig::Master(
                MasterConnectionInputConfig::gather_master_config(slave_configs)
            )
        } else {
            CentralizedSyncConnectionInputConfig::Slave(
                SlaveConnectionInputConfig::adapt_slave_config(master_config)
            )
        }
    };

    // Connect everything
    let mut connected_transfer = unconnected_transfer.connect(transfer_peer_config)?;
    let mut connected_sync = unconnected_sync.connect(sync_input_config)?;

    println!("Connected to transfer and sync networks.");

    // Perform 3 rounds of communication
    for round in 0..3 {
        println!("\n=== ROUND {} ===", round + 1);

        match role {
            Role::Sender(sender_id) => {
                println!("Sender {}: Starting round {}", sender_id, round + 1);

                // Send to all 3 receivers
                for receiver_idx in 0..3 {
                    let data_offset = round * 256; // 256 bytes per round
                    let data_range = data_offset..(data_offset + 256);

                    println!("Sender {}: Sending bytes {:?} to receiver {}",
                             sender_id, data_range, receiver_idx + 3);

                    let wc = connected_transfer.wait_send(receiver_idx, data_range).unwrap();
                    println!("Sender {}: Send to receiver {} completed: status={:?}",
                             sender_id, receiver_idx + 3, wc.is_valid());
                }

                println!("Sender {}: Finished sending in round {}", sender_id, round + 1);
            }
            Role::Receiver(receiver_id) => {
                println!("Receiver {}: Starting round {}", receiver_id, round + 1);

                // Receive from all 3 senders
                for sender_idx in 0..3 {
                    let data_offset = round * 256; // 256 bytes per round
                    let data_range = data_offset..(data_offset + 256);

                    println!("Receiver {}: Receiving bytes {:?} from sender {}",
                             receiver_id, data_range, sender_idx);

                    let wc = connected_transfer.wait_receive(sender_idx, data_range.clone()).unwrap();
                    println!("Receiver {}: Receive from sender {} completed: status={:?}",
                             receiver_id, sender_idx, wc.is_valid());

                    // Show first few bytes received
                    let start = data_range.start;
                    println!("Receiver {}: First 5 bytes from sender {} at offset {}: {:?}",
                             receiver_id, sender_idx, start,
                             &data_buffer[start..start + 5]);
                }

                println!("Receiver {}: Finished receiving in round {}", receiver_id, round + 1);
            }
        }

        // Synchronize all nodes before next round
        println!("Node {}: Waiting for barrier sync...", rank_id);
        if let Err(e) = connected_sync.wait_barrier() {
            eprintln!("Barrier failed: {e}");
            std::process::exit(1);
        }
        println!("Node {}: Barrier complete for round {}", rank_id, round + 1);
    }

    println!("\nNode {}: All rounds completed successfully!", rank_id);
    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Role {
    Sender(u32),
    Receiver(u32),
}

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
use infinibuilder::transfer::common::TransferConfig;
use infinibuilder::transfer::receiver::{
    ReceiverConnectionInputConfig, ReceiverConnectionOutputConfig, UnconnectedReceiverTransfer,
};
use infinibuilder::transfer::sender::{
    SenderConnectionInputConfig, SenderConnectionOutputConfig, UnconnectedSenderTransfer,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum NodeConfigs {
    Sender {
        transfer_config: SenderConnectionOutputConfig,
        sync_config: CentralizedSyncConnectionOutputConfig,
    },
    Receiver {
        transfer_config: ReceiverConnectionOutputConfig,
        sync_config: CentralizedSyncConnectionOutputConfig,
    },
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
        .add_node(TcpExchangerNodeConfig::new(0, "tdeb01".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(1, "tdeb02".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(2, "tdeb03".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(3, "tdeb05".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(4, "tdeb06".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(5, "tdeb07".to_string()))
        .unwrap();

    exchanger_network_config
        .get(rank_id)
        .expect("Rank id not in network");

    let role = match rank_id {
        0..=2 => Role::Sender(rank_id),
        3..=5 => Role::Receiver(rank_id),
        _ => panic!("Invalid rank ID, must be 0-5"),
    };

    // Open RDMA device
    let devices = ibverbs::devices()?;
    println!(
        "Devices: {:?}",
        devices.iter().map(|d| d.name()).collect::<Vec<_>>()
    );
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
        println!(
            "Sender {}: Initialized buffer with unique pattern",
            sender_id
        );
    }

    // Configure transfer and sync based on role
    let transfer_config = unsafe { TransferConfig::new(3, &data_buffer) };

    let (local_configs, connected_transfer_and_sync) = match role {
        Role::Sender(_) => {
            // Create sender transfer
            let unconnected_transfer = UnconnectedSenderTransfer::new(&context, transfer_config)?;
            let transfer_conn_cfg = unconnected_transfer.connection_config();

            // Configure sync (centralized with rank 0 as master)
            let sync_config = if rank_id == 0 {
                CentralizedSyncConfig::new_master(5) // 5 slaves
            } else {
                CentralizedSyncConfig::new_slave((rank_id - 1) as usize) // slaves indexed 0-4
            };
            let unconnected_sync = UnconnectedCentralizedSync::new(&context, sync_config)?;
            let sync_conn_cfg = unconnected_sync.connection_config();

            let local_configs = NodeConfigs::Sender {
                transfer_config: transfer_conn_cfg,
                sync_config: sync_conn_cfg,
            };

            (
                local_configs,
                (Some(unconnected_transfer), None, unconnected_sync),
            )
        }
        Role::Receiver(_) => {
            // Create receiver transfer
            let unconnected_transfer = UnconnectedReceiverTransfer::new(&context, transfer_config)?;
            let transfer_conn_cfg = unconnected_transfer.connection_config();

            // Configure sync (all receivers are slaves)
            let sync_config = CentralizedSyncConfig::new_slave((rank_id - 1) as usize);
            let unconnected_sync = UnconnectedCentralizedSync::new(&context, sync_config)?;
            let sync_conn_cfg = unconnected_sync.connection_config();

            let local_configs = NodeConfigs::Receiver {
                transfer_config: transfer_conn_cfg,
                sync_config: sync_conn_cfg,
            };

            (
                local_configs,
                (None, Some(unconnected_transfer), unconnected_sync),
            )
        }
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
    )
    .unwrap();

    println!(
        "Exchanged configs from {} nodes",
        exchanged_configs.as_slice().len()
    );

    // Connect based on role
    let (mut connected_transfer, mut connected_sync) = match role {
        Role::Sender(_) => {
            let (Some(unconnected_transfer), _, unconnected_sync) = connected_transfer_and_sync
            else {
                panic!("Expected sender transfer");
            };

            // Senders connect to receivers (ranks 3,4,5)
            let receiver_configs: Vec<_> = exchanged_configs
                .iter()
                .filter(|node| node.node_id >= 3 && node.node_id <= 5)
                .filter_map(|node| {
                    if let NodeConfigs::Receiver {
                        transfer_config, ..
                    } = &node.data
                    {
                        Some(transfer_config.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let sender_input_config = SenderConnectionInputConfig::gather_from_receivers(
                receiver_configs,
                rank_id as usize,
            )
            .expect("Failed to gather receiver configs");

            let connected_transfer = unconnected_transfer.connect(sender_input_config)?;

            // Set up sync connections (keeping original logic)
            let slave_configs: Vec<_> = exchanged_configs
                .iter()
                .filter_map(|node| {
                    let sync_config = match &node.data {
                        NodeConfigs::Sender { sync_config, .. } => sync_config,
                        NodeConfigs::Receiver { sync_config, .. } => sync_config,
                    };
                    if let CentralizedSyncConnectionOutputConfig::Slave(slave) = sync_config {
                        Some(slave.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let master_config = exchanged_configs
                .iter()
                .find_map(|node| {
                    let sync_config = match &node.data {
                        NodeConfigs::Sender { sync_config, .. } => sync_config,
                        NodeConfigs::Receiver { sync_config, .. } => sync_config,
                    };
                    if let CentralizedSyncConnectionOutputConfig::Master(master) = sync_config {
                        Some(master.clone())
                    } else {
                        None
                    }
                })
                .expect("No master config found");

            let sync_input_config = if rank_id == 0 {
                CentralizedSyncConnectionInputConfig::Master(
                    MasterConnectionInputConfig::gather_master_config(slave_configs),
                )
            } else {
                CentralizedSyncConnectionInputConfig::Slave(
                    SlaveConnectionInputConfig::adapt_slave_config(master_config),
                )
            };

            let connected_sync = unconnected_sync.connect(sync_input_config)?;

            (TransferType::Sender(connected_transfer), connected_sync)
        }
        Role::Receiver(_) => {
            let (_, Some(unconnected_transfer), unconnected_sync) = connected_transfer_and_sync
            else {
                panic!("Expected receiver transfer");
            };

            // Receivers connect to senders (ranks 0,1,2)
            let sender_configs: Vec<_> = exchanged_configs
                .iter()
                .filter(|node| node.node_id <= 2)
                .filter_map(|node| {
                    if let NodeConfigs::Sender {
                        transfer_config, ..
                    } = &node.data
                    {
                        Some(transfer_config.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let remote_idx = (rank_id - 3) as usize; // Map 3->0, 4->1, 5->2
            let receiver_input_config =
                ReceiverConnectionInputConfig::gather_from_senders(sender_configs, remote_idx)
                    .expect("Failed to gather sender configs");

            let connected_transfer = unconnected_transfer.connect(receiver_input_config)?;

            // Set up sync connections (same logic as sender)
            let slave_configs: Vec<_> = exchanged_configs
                .iter()
                .filter_map(|node| {
                    let sync_config = match &node.data {
                        NodeConfigs::Sender { sync_config, .. } => sync_config,
                        NodeConfigs::Receiver { sync_config, .. } => sync_config,
                    };
                    if let CentralizedSyncConnectionOutputConfig::Slave(slave) = sync_config {
                        Some(slave.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let master_config = exchanged_configs
                .iter()
                .find_map(|node| {
                    let sync_config = match &node.data {
                        NodeConfigs::Sender { sync_config, .. } => sync_config,
                        NodeConfigs::Receiver { sync_config, .. } => sync_config,
                    };
                    if let CentralizedSyncConnectionOutputConfig::Master(master) = sync_config {
                        Some(master.clone())
                    } else {
                        None
                    }
                })
                .expect("No master config found");

            let sync_input_config = CentralizedSyncConnectionInputConfig::Slave(
                SlaveConnectionInputConfig::adapt_slave_config(master_config),
            );

            let connected_sync = unconnected_sync.connect(sync_input_config)?;

            (TransferType::Receiver(connected_transfer), connected_sync)
        }
    };

    println!("Connected to transfer and sync networks.");

    // Perform 3 rounds of communication with proper ordering
    for round in 0..3 {
        println!("\n=== ROUND {} ===", round + 1);

        // PHASE 1: Receivers post receives (don't wait yet)
        let mut receive_requests = Vec::new();
        if let (Role::Receiver(receiver_id), TransferType::Receiver(receiver_transfer)) =
            (&role, &mut connected_transfer)
        {
            println!(
                "Receiver {}: Posting receives for round {}",
                receiver_id,
                round + 1
            );
            // Post receives from all 3 senders (but don't wait yet)
            for sender_idx in 0..3 {
                let data_offset = round * 256; // 256 bytes per round
                let data_range = data_offset..(data_offset + 256);
                println!(
                    "Receiver {}: Posting receive for bytes {:?} from sender {}",
                    receiver_id, data_range, sender_idx
                );

                let request = receiver_transfer
                    .post_receive(sender_idx, data_range)
                    .unwrap();
                receive_requests.push((sender_idx, request));
            }
            println!(
                "Receiver {}: All receives posted for round {}",
                receiver_id,
                round + 1
            );
        }

        // BARRIER 1: Ensure all receivers have posted their receives
        println!("Node {}: Waiting for receive-posting barrier...", rank_id);
        if let Err(e) = connected_sync.wait_barrier() {
            eprintln!("Receive-posting barrier failed: {e}");
            std::process::exit(1);
        }
        println!("Node {}: Receive-posting barrier complete", rank_id);

        // PHASE 2: Senders perform sends
        if let (Role::Sender(sender_id), TransferType::Sender(sender_transfer)) =
            (&role, &mut connected_transfer)
        {
            println!(
                "Sender {}: Starting sends for round {}",
                sender_id,
                round + 1
            );
            // Send to all 3 receivers
            for receiver_idx in 0..3 {
                let data_offset = round * 256; // 256 bytes per round
                let data_range = data_offset..(data_offset + 256);
                println!(
                    "Sender {}: Sending bytes {:?} to receiver {}",
                    sender_id,
                    data_range,
                    receiver_idx + 3
                );
                let wc = sender_transfer.wait_send(receiver_idx, data_range).unwrap();
                println!(
                    "Sender {}: Send to receiver {} completed: status={:?}",
                    sender_id,
                    receiver_idx + 3,
                    wc.is_valid()
                );
            }
            println!(
                "Sender {}: All sends completed for round {}",
                sender_id,
                round + 1
            );
        }

        // PHASE 3: Receivers wait for their posted receives to complete
        if let Role::Receiver(receiver_id) = role {
            println!(
                "Receiver {}: Waiting for receives to complete in round {}",
                receiver_id,
                round + 1
            );
            // Now wait for all the posted receives to complete
            for (sender_idx, request) in receive_requests {
                let wc = request.wait().unwrap();
                println!(
                    "Receiver {}: Receive from sender {} completed: status={:?}",
                    receiver_id,
                    sender_idx,
                    wc.is_valid()
                );

                // Show first few bytes received
                let data_offset = round * 256;
                println!(
                    "Receiver {}: First 5 bytes from sender {} at offset {}: {:?}",
                    receiver_id,
                    sender_idx,
                    data_offset,
                    &data_buffer[data_offset..data_offset + 5]
                );
            }
            println!(
                "Receiver {}: All receives completed for round {}",
                receiver_id,
                round + 1
            );
        }

        // BARRIER 2: Ensure all operations complete before next round
        println!("Node {}: Waiting for round completion barrier...", rank_id);
        if let Err(e) = connected_sync.wait_barrier() {
            eprintln!("Round completion barrier failed: {e}");
            std::process::exit(1);
        }
        println!("Node {}: Round {} completed", rank_id, round + 1);
    }

    println!("\nNode {}: All rounds completed successfully!", rank_id);
    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Role {
    Sender(u32),
    Receiver(u32),
}

enum TransferType {
    Sender(infinibuilder::transfer::sender::SenderTransfer),
    Receiver(infinibuilder::transfer::receiver::ReceiverTransfer),
}

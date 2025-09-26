use infinibuilder::tcp_exchanger::{TcpExchanger, TcpExchangerConfig, TcpExchangerNetworkConfig};
use infinibuilder::connect::Connect;
use infinibuilder::ibverbs::simple_unit::IbvSimpleUnit;
use infinibuilder::network::{IBNetwork, IBNetworkBuilder, IBNodeBuilderConfig, IBNodeRole};
use infinibuilder::rdma_traits::{RdmaRendezvous, WorkRequest};
use infinibuilder::rdma_traits::{RdmaSendRecv, WorkCompletion};
use std::env;
use std::io::{Read, Write};
use std::ops::RangeBounds;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn main() {
    let args = parse_args();

    let network = network();
    let node_idx = network.node(args.node_id.as_str()).unwrap().idx;
    let exchanger_network = TcpExchangerNetworkConfig::from_network(network).unwrap();

    let ibv_context = ibverbs::devices().unwrap().get(0).unwrap().open().unwrap();
    let memory = vec![170; args.message_length];

    let conn =
        unsafe { IbvSimpleUnit::new_sync_transfer_unit::<64, 64>(&ibv_context, &memory).unwrap() };

    // Serialize local config → JSON
    let local_config = conn.connection_config();
    let local_config_json = serde_json::to_string(&local_config).unwrap();

    // Exchange config
    let remote_config_json = TcpExchanger::await_exchange_network_config(
        node_idx,
        &local_config_json,
        &exchanger_network,
        &exchanger_config(),
    )
    .unwrap()
    .as_slice()[if node_idx == 0 { 1 } else { 0 }]
    .data()
    .clone();

    // Read remote config JSON from stdin
    let remote_config = serde_json::from_str(&remote_config_json).unwrap();

    let mut conn = conn.connect(remote_config).unwrap();

    println!("\n\n");

    let message_length = args.message_length; // example
    let num_messages = args.num_messages;
    let mr_range = 0..message_length;

    match args.mode {
        Mode::SpinSender => benchmark(
            || spin_send(&mut conn, mr_range.clone()),
            message_length,
            num_messages,
            "SpinSender",
        ),
        Mode::SpinReceiver => benchmark(
            || spin_receive(&mut conn, mr_range.clone()),
            message_length,
            num_messages,
            "SpinReceiver",
        ),
        Mode::SyncSender => benchmark(
            || sync_send(&mut conn, mr_range.clone()),
            message_length,
            num_messages,
            "SyncSender",
        ),
        Mode::SyncReceiver => benchmark(
            || sync_receive(&mut conn, mr_range.clone()),
            message_length,
            num_messages,
            "SyncReceiver",
        ),
    }
    .unwrap();
}

fn benchmark<F>(
    mut action: F,
    msg_size: usize,
    n_messages: usize,
    label: &str,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
{
    let start = Instant::now();
    let mut bytes = 0u64;

    for _ in 0..n_messages {
        action()?;
        bytes += msg_size as u64;
    }

    let elapsed = start.elapsed().as_secs_f64();
    let gbps = (bytes as f64 * 8.0) / elapsed / 1e9;
    println!(
        "{}: transferred {} bytes in {:.6} s → {:.2} Gbps",
        label, bytes, elapsed, gbps
    );

    Ok(())
}

fn spin_send<C: RdmaSendRecv, R: RangeBounds<usize> + Clone>(
    conn: &mut C,
    mr_range: R,
) -> std::io::Result<()> {
    loop {
        let wr = unsafe { conn.post_send(mr_range.clone(), None)? };
        if let Ok(_) = wr.wait() {
            return Ok(());
        }
    }
}

fn spin_receive<C: RdmaSendRecv, R: RangeBounds<usize> + Clone>(
    conn: &mut C,
    mr_range: R,
) -> std::io::Result<()> {
    loop {
        let wr = unsafe { conn.post_receive(mr_range.clone())? };
        if let Ok(_) = wr.wait() {
            return Ok(());
        }
    }
}

fn sync_send<C: RdmaSendRecv + RdmaRendezvous, R: RangeBounds<usize> + Clone>(
    conn: &mut C,
    mr_range: R,
) -> std::io::Result<()> {
    conn.rendezvous()?;
    unsafe { conn.post_send(mr_range, None)?.wait()? };
    Ok(())
}

fn sync_receive<C: RdmaSendRecv + RdmaRendezvous, R: RangeBounds<usize> + Clone>(
    conn: &mut C,
    mr_range: R,
) -> std::io::Result<()> {
    let wr = unsafe { conn.post_receive(mr_range)? };
    conn.rendezvous()?;
    wr.wait()?;
    Ok(())
}

#[derive(Debug, Copy, Clone)]
enum Mode {
    SpinSender,
    SpinReceiver,
    SyncSender,
    SyncReceiver,
}

struct Args {
    node_id: String,
    num_messages: usize,
    message_length: usize,
    mode: Mode,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();

    if args.len() != 5 {
        eprintln!(
            "Usage: {} node_id num_messages message_length [spin_sender|spin_receiver|sync_sender|sync_receiver]",
            args[0]
        );
        std::process::exit(1);
    }

    let node_id = args[1].clone();
    let num_messages = args[2].parse().unwrap();
    let message_length = args[3].parse().unwrap();

    let mode = match args[4].as_str() {
        "spin_sender" => Mode::SpinSender,
        "spin_receiver" => Mode::SpinReceiver,
        "sync_sender" => Mode::SyncSender,
        "sync_receiver" => Mode::SyncReceiver,
        other => {
            eprintln!(
                "Invalid mode: {}. Use 'spin_sender', 'spin_receiver', 'sync_sender' or 'sync_receiver'",
                other
            );
            std::process::exit(1);
        }
    };

    Args {
        node_id,
        num_messages,
        message_length,
        mode,
    }
}

fn network() -> IBNetwork<&'static str> {
    let mut network_builder = IBNetworkBuilder::new();
    network_builder.insert_node(
        "A",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb01".to_string(),
            port: 8000,
        },
    );
    network_builder.insert_node(
        "B",
        IBNodeBuilderConfig {
            role: IBNodeRole::Sender,
            address: "tdeb05".to_string(),
            port: 8001,
        },
    );
    network_builder.build()
}

fn exchanger_config() -> TcpExchangerConfig {
    TcpExchangerConfig {
        send_timeout: Duration::from_secs(60),
        send_attempt_delay: Duration::from_secs(1),
        receive_timeout: Duration::from_secs(60),
        receive_connection_timeout: Duration::from_secs(1),
    }
}

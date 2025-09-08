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
use std::env;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rank_id>", args[0]);
        std::process::exit(1);
    }

    let rank_id: u32 = args[1].parse().expect("Invalid rank ID");

    let exchanger_network_config = TcpExchangerNetworkConfig::new()
        .add_node(TcpExchangerNodeConfig::new(0, "tdeb01".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(1, "tdeb02".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(2, "tdeb03".to_string()))
        .unwrap()
        .add_node(TcpExchangerNodeConfig::new(3, "tdeb05".to_string()))
        .unwrap();

    exchanger_network_config
        .get(&rank_id)
        .expect("Rank id not in network");

    let mode = if rank_id == 0 {
        Mode::Master
    } else {
        Mode::Slave(rank_id as usize - 1)
    };

    // Open device/context
    let devices = ibverbs::devices()?;
    println!(
        "Devices: {:?}",
        devices.iter().map(|d| d.name()).collect::<Vec<_>>()
    );
    let context = devices
        .get(0)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No RDMA devices found"))?
        .open()?;

    // Build config
    let config = match mode {
        Mode::Master => CentralizedSyncConfig::new_master(exchanger_network_config.len() - 1),
        Mode::Slave(idx) => CentralizedSyncConfig::new_slave(idx),
    };

    // Create unconnected endpoint and print local connection config as JSON
    let unconnected = UnconnectedCentralizedSync::new(context, config)?;
    let local_conn_cfg = unconnected.connection_config();

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
    )
    .unwrap();

    println!("{:?}", exchanged_configs.as_slice());

    let slave_configs = exchanged_configs
        .iter()
        .map(|node| node.data.clone())
        .filter_map(|data| match data {
            CentralizedSyncConnectionOutputConfig::Master(master) => None,
            CentralizedSyncConnectionOutputConfig::Slave(slave) => Some(slave),
        })
        .collect::<Vec<_>>();

    let master_configs = exchanged_configs
        .iter()
        .map(|node| node.data.clone())
        .filter_map(|data| match data {
            CentralizedSyncConnectionOutputConfig::Master(master) => Some(master),
            CentralizedSyncConnectionOutputConfig::Slave(slave) => None,
        })
        .collect::<Vec<_>>();

    if master_configs.len() > 1 || master_configs.is_empty() {
        panic!("There must be exactly one master node");
    }

    let master_config = master_configs.first().unwrap().to_owned();

    // Convert into the typed input config based on our role
    let input_cfg: CentralizedSyncConnectionInputConfig = match mode {
        Mode::Master => CentralizedSyncConnectionInputConfig::Master(
            MasterConnectionInputConfig::gather_master_config(slave_configs),
        ),
        Mode::Slave(idx) => CentralizedSyncConnectionInputConfig::Slave(
            SlaveConnectionInputConfig::adapt_slave_config(master_config),
        ),
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Local output config and role mismatch",
            ));
        }
    };

    // Connect
    let mut connected = unconnected.connect(input_cfg)?;

    println!("Connected.");

    wait_key_barrier(&mut connected);

    wait_key_barrier(&mut connected);

    Ok(())
}

fn wait_key_barrier(sync: &mut impl SyncComponent) {
    println!("Press a key to sync barrier...");
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input).unwrap();
    println!("Waiting for the rest...");

    if let Err(e) = sync.wait_barrier() {
        eprintln!("Barrier failed: {e}");
        std::process::exit(1);
    }
    println!("Barrier complete.");
}

#[derive(Debug, Copy, Clone)]
enum Mode {
    Master,
    Slave(usize),
}

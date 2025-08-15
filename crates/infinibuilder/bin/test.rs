use infinibuilder::centralized_sync::{
    CentralizedSyncConfig, CentralizedSyncMasterConfig, UnconnectedCentralizedSync,
};
use std::env;
use std::process::exit;
use std::str::FromStr;

fn main() {
    let mode = select_mode_from_args();
    if let Mode::Undetermined = mode {
        exit(1)
    }

    let devices = ibverbs::devices().unwrap();

    println!(
        "Devices: {:?}",
        devices.iter().map(|d| d.name()).collect::<Vec<_>>()
    );

    let context = devices.get(0).unwrap().open().unwrap();

    let config = match mode {
        Mode::Master(num_nodes) => CentralizedSyncConfig::new_master(num_nodes),
        Mode::Slave(slave_idx) => CentralizedSyncConfig::new_slave(slave_idx),
        Mode::Undetermined => unreachable!(),
    };

    let unconnected = UnconnectedCentralizedSync::new(context, config).unwrap();
    let connection_config = unconnected.connection_config();
    println!(
        "Connection configuration: {}",
        serde_json::to_string(&connection_config).unwrap()
    );
}

#[derive(Debug, Copy, Clone)]
enum Mode {
    Master(usize), // number of nodes
    Slave(usize),  // slave index
    Undetermined,
}

fn select_mode_from_args() -> Mode {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <master|slave> <number>", args[0]);
        exit(1);
    }

    // Parse the second argument as a number
    let number = usize::from_str(&args[2]).unwrap_or_else(|_| {
        eprintln!("Second argument must be a positive integer");
        exit(1);
    });

    match args[1].as_str() {
        "master" => Mode::Master(number),
        "slave" => Mode::Slave(number),
        _ => Mode::Undetermined,
    }
}

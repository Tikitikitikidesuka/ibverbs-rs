use serde_json::Value;
use std::env;
use std::io::{Read};
use std::process::exit;
use std::str::FromStr;
use infinibuilder::synchronization::centralized::common::{CentralizedSyncConfig, CentralizedSyncConnectionInputConfig, CentralizedSyncConnectionOutputConfig, UnconnectedCentralizedSync};
use infinibuilder::synchronization::centralized::master::{MasterConnectionInputConfig, MasterConnectionOutputConfig};
use infinibuilder::synchronization::centralized::slave::{SlaveConnectionInputConfig, SlaveConnectionOutputConfig};
use infinibuilder::synchronization::SyncComponent;

fn main() -> std::io::Result<()> {
    let mode = select_mode_from_args();
    if let Mode::Undetermined = mode {
        exit(1)
    }

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
        Mode::Master(num_nodes) => CentralizedSyncConfig::new_master(num_nodes),
        Mode::Slave(slave_idx) => CentralizedSyncConfig::new_slave(slave_idx),
        Mode::Undetermined => unreachable!(),
    };

    // Create unconnected endpoint and print local connection config as JSON
    let unconnected = UnconnectedCentralizedSync::new(context, config)?;
    let local_conn_cfg = unconnected.connection_config();

    match local_conn_cfg {
        CentralizedSyncConnectionOutputConfig::Master(config) => {
            let json = serde_json::to_string_pretty(&config)?;
            println!("=== Local connection configuration (copy/share this) ===");
            println!("{json}");
        }
        CentralizedSyncConnectionOutputConfig::Slave(config) => {
            let json = serde_json::to_string_pretty(&config)?;
            println!("=== Local connection configuration (copy/share this) ===");
            println!("{json}");
        }
    }

    // Prompt and read the counterpart JSON
    println!("\n=== Paste counterpart connection configuration JSON then press Ctrl-D/Ctrl-Z ===");

    // Convert into the typed input config based on our role
    let input_cfg: CentralizedSyncConnectionInputConfig = match mode {
        Mode::Master(num_slaves) => {
            println!("All the slave output configs one by one...");
            let mut slave_output_configs = vec![];
            for _ in 0..num_slaves {
                let json = read_stdin_to_json()?;

                // Expect Master input (many slave output configs)
                let config =
                    serde_json::from_value::<SlaveConnectionOutputConfig>(json).map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("Expected Slave output config; JSON did not match: {e}"),
                        )
                    })?;

                slave_output_configs.push(config);
            }

            CentralizedSyncConnectionInputConfig::Master(
                MasterConnectionInputConfig::gather_master_config(slave_output_configs),
            )
        }
        Mode::Slave(_) => {
            println!("The master's config...");
            let json = read_stdin_to_json()?;

            // Expect Slave input (from master output config)
            let master_config = serde_json::from_value::<MasterConnectionOutputConfig>(json)
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Expected Slave input config; JSON did not match: {e}"),
                    )
                })?;

            CentralizedSyncConnectionInputConfig::Slave(
                SlaveConnectionInputConfig::adapt_slave_config(master_config),
            )
        }
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
    println!("Press a key to sync barrier...");
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input).unwrap();

    if let Err(e) = connected.wait_barrier() {
        eprintln!("Barrier failed: {e}");
        exit(1);
    }
    println!("Barrier complete.");

    Ok(())
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

fn read_stdin_to_json() -> std::io::Result<Value> {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s)?;

    serde_json::from_str(&s).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Failed to parse JSON: {e}"),
        )
    })
}

use infinibuilder::IbBNodeTcpQpEndpointExchanger;
use infinibuilder::IbBNodeTcpQpEndpointExchangeError::ConnectionError;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::exit;
use std::time::Duration;
use std::{env, io, thread};

#[derive(Debug, Copy, Clone)]
enum Mode {
    Server,
    Client,
    Undetermined,
}

fn select_mode_from_args() -> Mode {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <server|client>", args[0]);
        return Mode::Undetermined;
    }

    match args[1].as_str() {
        "server" => Mode::Server,
        "client" => Mode::Client,
        _ => {
            eprintln!("Invalid mode. Use 'server' or 'client'");
            Mode::Undetermined
        }
    }
}

fn prompt_for_address() -> io::Result<String> {
    print!("Enter server address: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

fn main() -> io::Result<()> {
    let mode = select_mode_from_args();

    if let Mode::Undetermined = mode {
        exit(1);
    }

    let server_qp = serde_json::from_str("{\"num\":14366,\"lid\":5,\"gid\":null}")?;
    let client_qp = serde_json::from_str("{\"num\":12384,\"lid\":2,\"gid\":null}")?;

    match mode {
        Mode::Server => {
            println!("Starting server mode...");

            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8844);
            let exchange = IbBNodeTcpQpEndpointExchanger::new(address).unwrap();
            println!("Running at: {address}");

            // Exchange qp
            let qp = exchange.accept_and_exchange(
                server_qp,
                Duration::from_secs(10),
            )
            .unwrap();

            println!("Queue pair: {qp:?}")
        }
        Mode::Client => {
            println!("Starting client mode...");

            // Prompt for server address
            let address = prompt_for_address()?;

            // Exchange qp
            loop {
                match IbBNodeTcpQpEndpointExchanger::connect_and_exchange(
                    &address,
                    client_qp,
                    Duration::from_secs(10),
                ) {
                    Ok(qp) => {
                        println!("Queue pair: {qp:?}");
                        break;
                    }
                    Err(ConnectionError(_)) => {
                        println!("Error connecting to server. Waiting 1 sec...");
                        thread::sleep(Duration::from_secs(1));
                    }
                    Err(e) => {
                        println!("Error: {e}");
                        break;
                    }
                }
            }
        }
        Mode::Undetermined => unreachable!(),
    }

    Ok(())
}

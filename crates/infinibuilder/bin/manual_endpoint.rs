use crate::Mode::{Receive, Send, Undetermined};
use ibverbs::QueuePairEndpoint;
use infinibuilder::IbBEndpointBuilder;
use std::io::Write;
use std::process::exit;
use std::{env, io};

const MESSAGE: &[u8] = b"HOLA, MUNDO!";

fn main() {
    let mode = select_mode_from_args();
    if let Undetermined = mode {
        exit(1);
    }

    let devices = ibverbs::devices().unwrap();

    println!(
        "Devices: {:?}",
        devices.iter().map(|d| d.name()).collect::<Vec<_>>()
    );

    let context = devices.get(0).unwrap().open().unwrap();
    let mut memory = vec![0u8; MESSAGE.len()];
    let endpoint = IbBEndpointBuilder::new()
        .set_context(&context)
        .set_data_memory_region(unsafe { &mut *(memory.as_mut_slice() as *mut [u8]) })
        .set_completion_queue_size(16)
        .build().unwrap();

    print_local_endpoint_json(&endpoint.endpoint());

    let remote_endpoint = prompt_for_remote_endpoint().unwrap();
    let mut connection = endpoint.connect(remote_endpoint).unwrap();

    match mode {
        Send => {
            println!("Sending message...");
            memory.copy_from_slice(MESSAGE);
            connection.post_send(..).unwrap().wait().unwrap();
            println!("Sent!");
        }
        Receive => {
            println!("Waiting for message...");
            connection.post_receive(..).unwrap().wait().unwrap();
            println!("Received: {:?}", str::from_utf8(&memory).unwrap());
        }
        _ => unreachable!(),
    }
}

#[derive(Debug, Copy, Clone)]
enum Mode {
    Send,
    Receive,
    Undetermined,
}

fn select_mode_from_args() -> Mode {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <send|receive>", args[0]);
        exit(1);
    }

    match args[1].as_str() {
        "send" => Send,
        "receive" => Receive,
        _ => Undetermined,
    }
}

fn prompt_for_remote_endpoint() -> Result<QueuePairEndpoint, ()> {
    print!("Remote endpoint: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| {
        eprintln!("Failed to read input: {}", e);
    })?;
    input.trim().to_string();

    serde_json::from_str::<QueuePairEndpoint>(&input).map_err(|e| {
        eprintln!("Failed to deserialize input: {}", e);
    })
}

fn print_local_endpoint_json(endpoint: &QueuePairEndpoint) {
    match serde_json::to_string(endpoint) {
        Ok(json) => {
            println!("Endpoint: {}", json);
        }
        Err(e) => {
            eprintln!("Failed to serialize local endpoint: {}", e);
        }
    }
}

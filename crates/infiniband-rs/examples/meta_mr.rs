use infiniband_rs::channel::single_channel::SingleChannel;
use infiniband_rs::ibverbs::devices::open_device;
use infiniband_rs::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
use infiniband_rs::network::tcp_exchanger::{ExchangeConfig, Exchanger};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::io::Read;
use std::time::Duration;
use std::{env, process};

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <rank>", args[0]);
        process::exit(1);
    }

    let rank: usize = args[1].parse().unwrap();
    let primary = rank == 0;

    let ctx = open_device(DEVICE).unwrap();

    let prep_conn = SingleChannel::builder().context(&ctx).build().unwrap();
    let endpoint = prep_conn.endpoint();

    let endpoint = match primary {
        true => Exchanger::await_exchange_pair(
            true,
            ("tdeb05", 9000),
            &endpoint,
            &ExchangeConfig::default(),
        )
        .unwrap(),
        false => Exchanger::await_exchange_pair(
            false,
            ("tdeb05", 9000),
            &endpoint,
            &ExchangeConfig::default(),
        )
        .unwrap(),
    };

    let mut conn = prep_conn.handshake(endpoint).unwrap();

    if primary {
        let mut mem = [0u8; 1024];
        println!("before: {:?}", &mem[0..8]);
        std::io::stdin().read(&mut [0]).unwrap();
        let mr = unsafe { conn.register_shared_mr(&mut mem).unwrap() };
        while mem[0] == 0u8 {}
        println!("after: {:?}", &mem[0..8]);
    } else {
        let mut remote_mr = conn.accept_remote_mr(Duration::from_millis(3000)).unwrap();
        let mut mem = [1u8; 1024];
        let mr = conn.register_local_mr(&mut mem).unwrap();
        conn.write(WriteWorkRequest::new(
            &[mr.prepare_gather_element(&mem).unwrap()],
            remote_mr.as_slice_mut(),
        ))
        .unwrap();
    }

    /*
    println!("Sync epoch: {:?}", conn.get_sync_epoch());
    println!("Pres to inc sync epoch...");
    std::io::stdin().read(&mut [0]).unwrap();
    conn.sync_epoch().unwrap();
    println!("Waiting sync epoch...");
    while conn.get_sync_epoch() == 0 {}
    println!("Sync epoch: {:?}", conn.get_sync_epoch());
    */
}

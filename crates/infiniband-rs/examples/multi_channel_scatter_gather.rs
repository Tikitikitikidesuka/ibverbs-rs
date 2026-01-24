use infiniband_rs::channel::multi_channel::MultiChannel;
use infiniband_rs::ibverbs::devices::open_device;
use infiniband_rs::ibverbs::scatter_gather_element::ScatterElement;
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::io;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = open_device(DEVICE).unwrap();

    let multi_channel = MultiChannel::builder()
        .context(&ctx)
        .num_channels(3)
        .build()
        .unwrap();

    let endpoints = multi_channel.endpoints();
    let mut multi_channel = multi_channel.handshake(endpoints).unwrap();

    let mut mem = [0u8; 10];
    let mr = multi_channel.register_mr(&mut mem).unwrap();

    let (send_mem, recv_mem) = mem.split_at_mut(5);

    let scatter_sends = send_mem
        .chunks(1)
        .map(|chunk| vec![mr.prepare_scatter_element(chunk).unwrap()])
        .enumerate();
    let gather_receives = recv_mem
        .chunks_mut(1)
        .map(|chunk| vec![mr.prepare_gather_element(chunk).unwrap()])
        .enumerate();

    println!("Recv mem before: {recv_mem:?}");

    let result = multi_channel.scatter(scatter_sends);

    println!("Recv mem after: {recv_mem:?}");

    println!("{result:?}");
}

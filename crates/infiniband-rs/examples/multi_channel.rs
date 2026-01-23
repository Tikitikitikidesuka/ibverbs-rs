use infiniband_rs::ibverbs::devices::open_device;
use infiniband_rs::multi_channel::MultiChannel;
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;

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

    multi_channel.scope(|c| {
        c.post_send()
    });
}

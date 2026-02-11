use infiniband_rs::channel::Channel;
use infiniband_rs::channel::remote_mr_exchanger::RemoteMrExchanger;
use infiniband_rs::ibverbs;
use infiniband_rs::ibverbs::work::WriteWorkRequest;
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::time::Duration;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibverbs::open_device(DEVICE).unwrap();
    let pd = ctx.allocate_pd().unwrap();
    let channel = Channel::builder().pd(&pd).build().unwrap();
    let exchanger = RemoteMrExchanger::new(&pd).unwrap();

    let channel_endpoint = channel.endpoint();
    let exchanger_endpoint = exchanger.remote();
    let mut channel = channel.handshake(channel_endpoint).unwrap();
    let mut exchanger = exchanger.link_remote(exchanger_endpoint);

    let mut mem = [0u8; 8];
    let mr = unsafe { pd.register_shared_mr(mem.as_mut_ptr(), mem.len()).unwrap() };

    exchanger.share_memory_region(&mut channel, &mr).unwrap();
    let remote_mr = exchanger
        .accept_memory_region(&mut channel, Duration::from_secs(5))
        .unwrap();

    mem[0..4].fill(8);

    println!("Mem before exchange: {mem:?}");

    channel
        .write(WriteWorkRequest::new(
            &[mr.gather_element(&mem[0..4]).unwrap()],
            remote_mr.sub_region(4).unwrap(),
        ))
        .unwrap();

    println!("Mem after exchange: {mem:?}");
}

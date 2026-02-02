use infiniband_rs::channel::Channel;
use infiniband_rs::channel::remote_mr_exchanger::RemoteMrExchanger;
use infiniband_rs::ibverbs;
use infiniband_rs::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::io;
use std::time::Duration;

const DEVICE: &str = "mlx5_0";

fn main() -> io::Result<()> {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibverbs::open_device(DEVICE)?;
    let pd = ctx.allocate_pd()?;
    let channel = Channel::builder().pd(&pd).build()?;
    let exchanger = RemoteMrExchanger::new(&pd)?;

    let channel_endpoint = channel.endpoint();
    let exchanger_endpoint = exchanger.remote();
    let mut channel = channel.handshake(channel_endpoint)?;
    let mut exchanger = exchanger.link_remote(exchanger_endpoint);

    let mut mem = [0u8; 8];
    let mr = unsafe { pd.register_shared_mr(mem.as_mut_ptr(), mem.len())? };

    exchanger.share_memory_region(&mut channel, &mr)?;
    let remote_mr = exchanger.accept_memory_region(&mut channel, Duration::from_secs(5))?;

    mem[0..4].fill(8);

    println!("Mem before exchange: {mem:?}");

    channel.write(WriteWorkRequest::new(
        &[mr.gather_element(&mem[0..4]).unwrap()],
        remote_mr.sub_region(4).unwrap(),
    ))?;

    println!("Mem after exchange: {mem:?}");

    Ok(())
}

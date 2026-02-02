use ibverbs_sys::ibv_sge;
use infiniband_rs::channel::Channel;
use infiniband_rs::channel::polling_scope::ScopeError;
use infiniband_rs::ibverbs;
use infiniband_rs::ibverbs::scatter_gather_element::GatherElement;
use infiniband_rs::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::io;

const DEVICE: &str = "mlx5_0";

fn main() -> io::Result<()> {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibverbs::open_device(DEVICE)?;
    let pd = ctx.allocate_pd()?;
    let channel = Channel::builder().pd(&pd).build()?;

    let mut mem = [0u8; 8];
    let mr = pd.register_local_mr_slice(&mem)?;

    let endpoint = channel.endpoint();
    let mut channel = channel.handshake(endpoint)?;

    mem[0..4].fill(8);

    println!("Mem before exchange: {mem:?}");

    channel
        .scope(|s| {
            let (send_mem, recv_mem) = mem.split_at_mut(4);

            let send_ge = [mr.prepare_gather_element(send_mem).unwrap()];
            let mut recv_se = [mr.prepare_scatter_element(recv_mem).unwrap()];

            s.post_receive(ReceiveWorkRequest::new(&mut recv_se))?;
            s.post_send(SendWorkRequest::new(&send_ge))?;
            Ok::<(), io::Error>(())
        })
        .unwrap();

    println!("Mem after exchange: {mem:?}");

    Ok(())
}

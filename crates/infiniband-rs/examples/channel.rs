use infiniband_rs::channel::Channel;
use infiniband_rs::ibverbs;
use infiniband_rs::ibverbs::error::IbvError;
use infiniband_rs::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibverbs::open_device(DEVICE).unwrap();
    let pd = ctx.allocate_pd().unwrap();
    let channel = Channel::builder().pd(&pd).build().unwrap();

    let mut mem = [0u8; 8];
    let mr = pd.register_local_mr_slice(&mem).unwrap();

    let endpoint = channel.endpoint();
    let mut channel = channel.handshake(endpoint).unwrap();

    mem[0..4].fill(8);

    println!("Mem before exchange: {mem:?}");

    channel
        .scope(|s| {
            let (send_mem, recv_mem) = mem.split_at_mut(4);

            let send_ge = [mr.gather_element(send_mem).unwrap()];
            let mut recv_se = [mr.scatter_element(recv_mem).unwrap()];

            s.post_receive(ReceiveWorkRequest::new(&mut recv_se))?;
            s.post_send(SendWorkRequest::new(&send_ge))?;

            Ok(())
        })
        .unwrap();

    println!("Mem after exchange: {mem:?}");
}

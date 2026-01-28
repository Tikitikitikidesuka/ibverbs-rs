use infiniband_rs::channel::single_channel::SingleChannel;
use infiniband_rs::ibverbs::devices::open_device;
use infiniband_rs::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = open_device(DEVICE).unwrap();

    let prep_conn = SingleChannel::builder().context(&ctx).build().unwrap();
    let endpoint = prep_conn.endpoint();
    let mut conn = prep_conn.handshake(endpoint).unwrap();

    let mut mem = vec![0u8; 1024];
    let mr = conn.register_local_mr(&mut mem).unwrap();

    // Polling to completion
    println!("Running scoped connection and polling...");
    let (send_mem, rest) = mem.split_at_mut(4);
    let (recv_mem, _rest) = rest.split_at_mut(4);
    recv_mem.copy_from_slice(&[0, 0, 0, 0]);
    println!("before recv: {:?}", &recv_mem[0..4]);
    send_mem.copy_from_slice(&[1, 2, 3, 4]);
    conn.scope(|s| {
        let wr0 = s
            .post_receive(ReceiveWorkRequest::new(&mut [mr
                .prepare_scatter_element(recv_mem)
                .unwrap()]))
            .unwrap();
        let wr1 = s
            .post_send(SendWorkRequest::new(&[mr
                .prepare_gather_element(send_mem)
                .unwrap()]))
            .unwrap();
        wr0.spin_poll().unwrap();
        wr1.spin_poll().unwrap();
    })
    .unwrap();
    println!("after recv: {:?}", &recv_mem[0..4]);
}

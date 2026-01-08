use infiniband_rs::connection::builder::IbvConnectionBuilder;
use infiniband_rs::connection::connection::IbvConnPolledWrError;
use infiniband_rs::devices::ibv_device_list;
use simple_logger::SimpleLogger;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().init().unwrap();

    let devices = ibv_device_list().unwrap();
    println!("{devices:?}");

    let device = devices
        .iter()
        .find(|device| device.name() == Some(DEVICE))
        .unwrap();

    println!("{device:?}");

    let ctx = device.open().unwrap();

    let prep_conn = IbvConnectionBuilder::new(&ctx).build().unwrap();
    let endpoint = prep_conn.endpoint();
    let mut conn = prep_conn.handshake(endpoint).unwrap();

    let mut mem = vec![0u8; 1024];
    let mr = conn
        .register_mr("asdf", mem.as_mut_ptr(), mem.len())
        .unwrap();

    // Polling to completion
    println!("Running scoped connection and polling...");
    let (send_mem, rest) = mem.split_at_mut(4);
    let (recv_mem, _rest) = rest.split_at_mut(4);
    recv_mem.copy_from_slice(&[0, 0, 0, 0]);
    println!("before recv: {:?}", &recv_mem[0..4]);
    send_mem.copy_from_slice(&[1, 2, 3, 4]);
    conn.scope(|s| {
        let wr0 = s
            .post_receive(&[mr.prepare_receive(recv_mem).unwrap()])
            .unwrap();
        let wr1 = s.post_send(&[mr.prepare_send(send_mem).unwrap()]).unwrap();
        wr0.spin_poll().unwrap().unwrap();
        wr1.spin_poll().unwrap().unwrap();
    });
    println!("after recv: {:?}", &recv_mem[0..4]);

    // Forgetting to poll to completion
    println!("Running scoped connection skipping polls...");
    let (send_mem, rest) = mem.split_at_mut(4);
    let (recv_mem, _rest) = rest.split_at_mut(4);
    recv_mem.copy_from_slice(&[0, 0, 0, 0]);
    println!("before recv: {:?}", &recv_mem[0..4]);
    send_mem.copy_from_slice(&[1, 2, 3, 4]);
    conn.scope(|s| {
        let wr0 = s
            .post_receive(&[mr.prepare_receive(recv_mem).unwrap()])
            .unwrap();
        let wr1 = s.post_send(&[mr.prepare_send(send_mem).unwrap()]).unwrap();
    });
    println!("after recv: {:?}", &recv_mem[0..4]);

    // Returning spin poll result
    println!("Running scoped connection, polling and returning poll errors...");
    let (send_mem, rest) = mem.split_at_mut(4);
    let (recv_mem, _rest) = rest.split_at_mut(4);
    recv_mem.copy_from_slice(&[0, 0, 0, 0]);
    println!("before recv: {:?}", &recv_mem[0..4]);
    send_mem.copy_from_slice(&[1, 2, 3, 4]);
    let result: Result<(), IbvConnPolledWrError> = conn.scope(|s| {
        let wr0 = s
            .post_receive(&[mr.prepare_receive(recv_mem).unwrap()])
            .unwrap();
        let wr1 = s.post_send(&[mr.prepare_send(send_mem).unwrap()]).unwrap();
        // Since you can forget to poll and the work request will be polled
        // at the end of the scope, you may just return the errors with ?.
        wr0.spin_poll()??;
        wr1.spin_poll()??;
        Ok(())
    });
    println!("Result: {:?}", result);
    println!("after recv: {:?}", &recv_mem[0..4]);
}

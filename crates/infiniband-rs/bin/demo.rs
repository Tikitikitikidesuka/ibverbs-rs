use infiniband_rs::connection::builder::IbvConnectionBuilder;
use infiniband_rs::devices::ibv_device_list;

const DEVICE: &str = "mlx5_0";

fn main() {
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

    let mut memory = vec![0u8; 1024];
    let mr = conn
        .register_mr("asdf", memory.as_mut_ptr(), memory.len())
        .unwrap();

    //println!("{conn:?}");

    let (send_mem, recv_mem) = memory.split_at_mut(4);
    let mut recv_wr = unsafe {
        conn.receive_unpolled(&[mr.prepare_receive(recv_mem).unwrap()])
            .unwrap()
    };
    let mut send_wr = unsafe {
        conn.send_unpolled(&[mr.prepare_send(send_mem).unwrap()])
            .unwrap()
    };

    println!("Receive wr: {recv_wr:?}");
    println!("Send wr: {send_wr:?}");

    let recv_result = recv_wr.spin_poll().unwrap().unwrap();
    match recv_result {
        Ok(wc) => println!("Receive success: {wc:?}"),
        Err(we) => println!("Receive error: {we}"),
    };
    let send_result = send_wr.spin_poll().unwrap().unwrap();
    match send_result {
        Ok(wc) => println!("Send success: {wc:?}"),
        Err(we) => println!("Send error: {we}"),
    };

    /*
    conn.send(&[
        mr.prepare_send(&memory[0..4]).unwrap(),
        mr.prepare_send(&memory[4..8]).unwrap(),
        mr.prepare_send(&memory[8..12]).unwrap(),
        mr.prepare_send(&memory[12..16]).unwrap(),
    ])
    .unwrap();

    let (mem_0, rest) = memory.split_at_mut(4);
    let (mem_1, rest) = rest.split_at_mut(4);
    let (mem_2, rest) = rest.split_at_mut(4);
    let (mem_3, rest) = rest.split_at_mut(4);
    conn.receive(&mut [
        mr.prepare_receive(mem_0).unwrap(),
        mr.prepare_receive(mem_1).unwrap(),
        mr.prepare_receive(mem_2).unwrap(),
        mr.prepare_receive(mem_3).unwrap(),
    ])
    .unwrap();

    conn.send(vec![
        mr.prepare_send(&memory[0..4]).unwrap(),
        mr.prepare_send(&memory[4..8]).unwrap(),
        mr.prepare_send(&memory[8..12]).unwrap(),
        mr.prepare_send(&memory[12..16]).unwrap(),
    ])
    .unwrap();

    let (mem_0, rest) = memory.split_at_mut(4);
    let (mem_1, rest) = rest.split_at_mut(4);
    let (mem_2, rest) = rest.split_at_mut(4);
    let (mem_3, rest) = rest.split_at_mut(4);
    conn.receive(vec![
        mr.prepare_receive(mem_0).unwrap(),
        mr.prepare_receive(mem_1).unwrap(),
        mr.prepare_receive(mem_2).unwrap(),
        mr.prepare_receive(mem_3).unwrap(),
    ])
    .unwrap();
    */
}

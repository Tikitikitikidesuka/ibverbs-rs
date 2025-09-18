use infinibuilder::new_ib::simpleconn::{SimpleConnection, UnconnectedSimpleConnection};

fn main() -> std::io::Result<()> {
    let ib_context = ibverbs::devices()?.get(0).unwrap().open()?;
    let memory0: [u8; 256] = std::array::from_fn(|i| i as u8);
    let memory1 = [0u8; 256];

    let conn0 = unsafe { SimpleConnection::new::<64>(&ib_context, &memory0)? };
    let conn1 = unsafe { SimpleConnection::new::<64>(&ib_context, &memory1)? };

    let config0 = conn0.connection_config();
    let config1 = conn1.connection_config();

    let mut conn0 = conn0.connect(config1)?;
    let mut conn1 = conn1.connect(config0)?;

    /*
    let wr1 = unsafe { conn1.post_receive(4..20)? };
    let wr0 = unsafe { conn0.post_send(0..16, None)? };

    wr1.wait()?;
    wr0.wait()?;
    */

    let wr1 = conn1.post_write(32..34, 32..34, None)?;
    let wr0 = conn0.post_read(230..240, 240..250)?;

    wr1.wait()?;
    wr0.wait()?;

    println!("Memory 0: {memory0:?}");
    println!("Memory 1: {memory1:?}");

    Ok(())
}

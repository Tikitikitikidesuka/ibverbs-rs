use infinibuilder::connection::Connect;
use infinibuilder::ibverbs::simple_unit::IbvSimpleUnit;
use infinibuilder::rdma_traits::{RdmaReadWrite, RdmaSendRecv};
use infinibuilder::rdma_traits::{RdmaRendezvous, WorkRequest};
use simple_logger::SimpleLogger;
use std::thread;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    SimpleLogger::new().init().unwrap();

    let ibv_context = ibverbs::devices()?.get(0).unwrap().open()?;
    let memory0: [u8; 16] = std::array::from_fn(|i| i as u8);
    let memory1 = [0u8; 16];

    let conn0 = unsafe { IbvSimpleUnit::new_sync_transfer_unit::<64, 64>(&ibv_context, &memory0)? };
    let conn1 = unsafe { IbvSimpleUnit::new_sync_transfer_unit::<64, 64>(&ibv_context, &memory1)? };

    let config0 = conn0.connection_config();
    let config1 = conn1.connection_config();

    let mut conn0 = conn0.connect(config1)?;
    let mut conn1 = conn1.connect(config0)?;

    let wr1 = unsafe { conn1.post_receive(4..8)? };
    let wr0 = unsafe { conn0.post_send(0..4, None)? };

    println!("{:?}", wr1.wait()?);
    println!("{:?}", wr0.wait()?);

    let wr1 = unsafe { conn0.post_write(4..8, 8..12, None) }?;
    let wr0 = unsafe { conn1.post_read(12..16, 8..12) }?;

    println!("{:?}", wr1.wait()?);
    println!("{:?}", wr0.wait()?);

    let thread_handle = thread::spawn(move || -> std::io::Result<()> {
        thread::sleep(Duration::from_secs(2));
        println!("Connection 1 awaiting rendezvous");
        conn1.rendezvous()
    });

    println!("Connection 0 awaiting rendezvous");
    conn0.rendezvous()?;

    thread_handle.join().unwrap()?;

    println!("Memory 0: {memory0:?}");
    println!("Memory 1: {memory1:?}");

    Ok(())
}

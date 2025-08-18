use ibverbs::ibv_qp_type::IBV_QPT_RC;
use ibverbs::{ibv_access_flags, ibv_wc};

fn main() -> std::io::Result<()> {
    let context0 = ibverbs::devices()?.get(0).unwrap().open()?;
    let cq0 = context0.create_cq(32, 0)?;
    let pd0 = context0.alloc_pd()?;
    let mut mr0 = pd0.allocate(16)?;
    let qp0 = pd0
        .create_qp(&cq0, &cq0, IBV_QPT_RC)?
        .set_access(ibv_access_flags::IBV_ACCESS_REMOTE_WRITE)
        .build()?;
    let qpe0 = qp0.endpoint()?;

    let context1 = ibverbs::devices()?.get(1).unwrap().open()?;
    let cq1 = context1.create_cq(32, 0)?;
    let pd1 = context1.alloc_pd()?;
    let mut mr1 = pd1.allocate(16)?;
    let qp1 = pd1
        .create_qp(&cq1, &cq1, IBV_QPT_RC)?
        .set_access(ibv_access_flags::IBV_ACCESS_REMOTE_WRITE)
        .build()?;
    let qpe1 = qp1.endpoint()?;

    let mut qp0 = qp0.handshake(qpe1)?;
    let mut qp1 = qp1.handshake(qpe0)?;

    println!("MR0 rkey: {:?}", mr0.remote());
    println!("MR1 rkey: {:?}", mr1.remote());

    // Initialize data BEFORE operations
    mr0.inner().copy_from_slice(vec![1; 16].as_slice());
    println!("MR0: {:?}", mr0.inner());
    println!("MR1 before: {:?}", mr1.inner());

    // Just do RDMA write - no need for send/receive
    qp0.post_write(&[mr0.slice(..)], mr1.remote().slice(..), 33, None)?;

    let poll_buffer = &mut [ibv_wc::default(); 16];
    println!("Waiting for RDMA write completion...");

    let mut write_completed = false;
    while !write_completed {
        let completions = cq0.poll(poll_buffer)?;
        for completion in completions {
            println!(
                "Got completion: wr_id={}, status={:?}, opcode={:?}",
                completion.wr_id(),
                completion.is_valid(),
                completion.opcode()
            );

            if completion.wr_id() == 33 {
                if !completion.is_valid() {
                    panic!("RDMA write failed: {:?}", completion);
                }
                write_completed = true;
                break;
            }
        }
        if !write_completed {
            std::hint::spin_loop();
        }
    }

    println!("RDMA write completed successfully!");

    // Wait for data to appear in mr1
    println!("Waiting for data in MR1...");
    let mut retries = 0;
    while !mr1.inner().iter().all(|v| *v == 1) && retries < 1000 {
        std::hint::spin_loop();
        retries += 1;
    }

    println!("MR1 after: {:?}", mr1.inner());

    if mr1.inner().iter().all(|v| *v == 1) {
        println!("Success! Data was written from MR0 to MR1");
    } else {
        println!("Warning: Data may not have been fully written");
    }

    Ok(())
}

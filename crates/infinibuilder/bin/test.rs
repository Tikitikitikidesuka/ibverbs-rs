use infinibuilder::IbBEndpointBuilder;

fn main() {
    let devices = ibverbs::devices().unwrap();

    println!(
        "Devices: {:?}",
        devices.iter().map(|d| d.name()).collect::<Vec<_>>()
    );

    println!("Opening contexts...");
    let context_a = devices.get(0).unwrap().open().unwrap();
    let context_b = devices.get(1).unwrap().open().unwrap();

    println!("Creating memories...");
    let mut memory_a = [0u8, 0, 0, 0];
    let mut memory_b = [0u8, 1, 2, 3];

    println!("Decoupling memory unsafely for independent mutable reference");
    let rdma_mem_a = unsafe { &mut *(&mut memory_a[..] as *mut [u8]) };
    let rdma_mem_b = unsafe { &mut *(&mut memory_b[..] as *mut [u8]) };

    println!("Creating endpoints...");
    let endpoint_a = IbBEndpointBuilder::new()
        .set_context(&context_a)
        .set_data_memory_region(rdma_mem_a)
        .set_completion_queue_size(16)
        .build()
        .unwrap();
    let endpoint_b = IbBEndpointBuilder::new()
        .set_context(&context_b)
        .set_data_memory_region(rdma_mem_b)
        .set_completion_queue_size(16)
        .build()
        .unwrap();

    println!("Endpoint A: {:?}", endpoint_a.endpoint());
    println!("Endpoint B: {:?}", endpoint_b.endpoint());

    println!("Establishing connections...");
    let mut endpoint_a = endpoint_a.connect(endpoint_b.endpoint()).unwrap();
    let mut endpoint_b = endpoint_b.connect(endpoint_a.endpoint()).unwrap();

    println!("Connection established");

    println!("Memory A: {:?}", memory_a);
    println!("Memory B: {:?}", memory_b);

    let receive_wr = endpoint_a.post_receive(..).unwrap();
    let extra_receive_wr = endpoint_a.post_receive(..).unwrap();
    let send_wr = endpoint_b.post_send(1..2).unwrap();

    println!("Waiting for receive wr...");
    receive_wr.wait().unwrap();

    println!("Memory A: {:?}", memory_a);
    println!("Memory B: {:?}", memory_b);

    //extra_receive_wr.wait();
}

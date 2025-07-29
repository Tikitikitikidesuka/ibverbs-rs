use infinibuilder::EndpointBuilder;

fn main() {
    let devices = ibverbs::devices().unwrap();
    let context = devices.get(0).unwrap().open().unwrap();

    let mut memory_a = [0u8; 1024];
    let mut memory_b = [0u8; 1024];

    let builder_a = EndpointBuilder::new(&context).set_data_memory_region(&mut memory_a);
    let builder_b = EndpointBuilder::new(&context).set_data_memory_region(&mut memory_b);
    /*
    let endpoint_a = builder_a.endpoint();
    let endpoint_b = builder_b.endpoint();
    builder_a.connect(endpoint_b);
    builder_b.connect(endpoint_a);

     */
}

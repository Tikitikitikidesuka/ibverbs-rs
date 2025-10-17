use infinibuilder::restructure::ibverbs::network_node::IbvNetworkNodeBuilder;

fn main() {
    let node_builder = IbvNetworkNodeBuilder::new()
        .with_ibv_device("mlx5_0")
        .unwrap()
        .set_completion_queue_params(32, 512)
        .create_connections(4)
        .unwrap();
}

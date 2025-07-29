use ibverbs::ibv_access_flags;
use ibverbs::ibv_qp_type::IBV_QPT_RC;

fn main() {
    let context = ibverbs::devices()
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .open()
        .unwrap();

    let cq = context.create_cq(16, 0).unwrap();
    let pd = context.alloc_pd().unwrap();

    let qp_builder = pd.create_qp(&cq, &cq, IBV_QPT_RC)
        .unwrap()
        //.set_gid_index(1)
        .build()
        .unwrap();

    let endpoint = qp_builder.endpoint().unwrap();
    let mut qp = qp_builder.handshake(endpoint).unwrap();

    let mut mr = pd.allocate(256).unwrap();
    let mem = mr.inner().as_mut_slice();
}

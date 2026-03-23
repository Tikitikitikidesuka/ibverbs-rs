# ibverbs-rs

[![Crates.io](https://img.shields.io/crates/v/ibverbs-rs)](https://crates.io/crates/ibverbs-rs)
[![Docs](https://docs.rs/ibverbs-rs/badge.svg)](https://docs.rs/ibverbs-rs)
[![License](https://img.shields.io/crates/l/ibverbs-rs)](https://github.com/Tikitikitikidesuka/ibverbs-rs/blob/main/LICENSE-MIT)

Safe, ergonomic Rust bindings for the InfiniBand **libibverbs** API.

`ibverbs-rs` provides high-level abstractions for RDMA (Remote Direct Memory Access)
programming while preserving the safety guarantees of Rust where possible.
Built on top of [`ibverbs-sys`](https://crates.io/crates/ibverbs-sys).

## Features

- **Device discovery** — enumerate InfiniBand devices and open contexts
- **Memory registration** — safe local memory regions; explicit `unsafe` for remotely-accessible regions
- **Channel** — a single point-to-point RDMA connection with scope-based completion polling
- **MultiChannel** — multiple parallel connections sharing a protection domain, with scatter/gather support
- **Network coordination** — TCP-based endpoint exchange, distributed barriers (linear, binary tree, dissemination)
- **NUMA awareness** — optional thread-to-NUMA pinning (enable the `numa` feature)

## Safety model

Two-sided operations (send/receive) are lifetime-safe through Rust's borrow checker —
the data buffers are borrowed for the duration of the operation.

One-sided operations (RDMA read/write) require `unsafe` on the passive side because the
remote peer can access registered memory at any time without local coordination.

See the [`ibverbs::memory`](https://docs.rs/ibverbs-rs/latest/ibverbs_rs/ibverbs/memory/index.html)
module documentation for a detailed explanation.

## Requirements

- Linux with RDMA-capable hardware (InfiniBand or RoCE)
- `rdma-core` development libraries (`rdma-core-devel` on RHEL/Alma, `rdma-core` on Debian/Ubuntu)
- Rust 2024 edition (nightly or stable 1.85+)

## Quick start

```rust
use ibverbs_rs::ibverbs;

// Open a device and set up a protection domain
let ctx = ibverbs::open_device("mlx5_0").unwrap();
let pd = ctx.allocate_pd().unwrap();

// Register memory and create a channel
let mut buf = [0u8; 64];
let mr = pd.register_local_mr_slice(&buf).unwrap();
let channel = ibverbs_rs::channel::Channel::builder()
    .pd(&pd)
    .build()
    .unwrap();
```

See the [`examples/`](examples/) directory for complete working programs including
point-to-point channels, multi-channel scatter/gather, and distributed network barriers.

## Optional features

| Feature | Description                                         |
|---------|-----------------------------------------------------|
| `numa`  | Enables NUMA-aware thread pinning (links `libnuma`) |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

Developed by Miguel Hermoso Mantecón and Jonatan Ziegler.

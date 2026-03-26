# ibverbs-rs

[![Crates.io](https://img.shields.io/crates/v/ibverbs-rs)](https://crates.io/crates/ibverbs-rs)
[![Docs](https://docs.rs/ibverbs-rs/badge.svg)](https://docs.rs/ibverbs-rs)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](https://github.com/Tikitikitikidesuka/ibverbs-rs/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Tikitikitikidesuka/ibverbs-rs/blob/main/LICENSE-APACHE)

Safe, ergonomic Rust bindings for the InfiniBand **libibverbs** API.

RDMA programming in C is notoriously error-prone: dangling buffers, use-after-free on
memory regions, and silent data corruption from reusing memory mid-DMA are all common
pitfalls. `ibverbs-rs` eliminates these classes of bugs at compile time by encoding
RDMA safety invariants in the type system and borrow checker, without sacrificing the
zero-copy performance that makes RDMA worth using in the first place.

Built on top of [`ibverbs-sys`](https://crates.io/crates/ibverbs-sys).

## Features

- **Device discovery** — enumerate InfiniBand devices and open contexts.
- **Memory registration** — safe local memory regions; explicit `unsafe` for remotely-accessible regions.
- **Channel** — a single point-to-point RDMA connection with scope-based completion polling.
- **MultiChannel** — multiple parallel connections sharing a protection domain, with scatter/gather support.
- **Network coordination** — TCP-based endpoint exchange, distributed barriers (linear, binary tree, dissemination).
- **NUMA awareness** — optional thread-to-NUMA pinning (enable the `numa` feature).

## Getting started

Choose the abstraction level that fits your use case:

* **Single connection** — the [`channel`](https://docs.rs/ibverbs-rs/latest/ibverbs_rs/channel/index.html) module provides a fully memory-safe point-to-point RDMA connection.
* **Multiple peers** — the [`multi_channel`](https://docs.rs/ibverbs-rs/latest/ibverbs_rs/multi_channel/index.html) module provides multiple indexed channels sharing memory regions.
* **Distributed network** — the [`network`](https://docs.rs/ibverbs-rs/latest/ibverbs_rs/network/index.html) module sets up a ranked RDMA network
  with barrier synchronization. Includes an out-of-band TCP exchanger for easy
  endpoint discovery and cluster bootstrapping.
* **Low-level control** — the [`ibverbs`](https://docs.rs/ibverbs-rs/latest/ibverbs_rs/ibverbs/index.html) module exposes the raw primitives (devices,
  protection domains, queue pairs, completion queues, memory regions, and work requests)
  for when you need full control over the RDMA stack.

## Safety model

Two-sided operations (send/receive) are checked by Rust's borrow checker —
the data buffers are borrowed for the duration of the operation, so the compiler
rejects any attempt to read, mutate, or drop a buffer while the NIC may still be
performing DMA on it.

One-sided operations (RDMA read/write) require `unsafe` on the passive side because the
remote peer can access registered memory at any time without local coordination.

## Quick start

Open a device, build a channel, and exchange data over a loopback connection:

```rust,no_run
use ibverbs_rs::ibverbs;
use ibverbs_rs::channel::{Channel, ScopeError, TransportError};
use ibverbs_rs::ibverbs::work::{SendWorkRequest, ReceiveWorkRequest};

// Open device and allocate resources
let ctx = ibverbs::open_device("mlx5_0")?;
let pd = ctx.allocate_pd()?;

// Build a channel and perform loopback handshake
let prepared = Channel::builder().pd(&pd).build()?;
let endpoint = prepared.endpoint();
let mut channel = prepared.handshake(endpoint)?;

// Register a buffer — first half for sending, second half for receiving
let mut buf = [0u8; 8];
buf[0..4].copy_from_slice(&[1, 2, 3, 4]);
let mr = pd.register_local_mr_slice(&buf)?;

// Scoped send + receive: the borrow checker ensures `buf` cannot be
// accessed until both operations complete
channel.scope(|s| {
    let (tx, rx) = buf.split_at_mut(4);

    s.post_receive(ReceiveWorkRequest::new(&mut [mr.scatter_element(rx)]))?;
    s.post_send(SendWorkRequest::new(&[mr.gather_element(tx)]))?;

    Ok::<(), ScopeError<TransportError>>(())
})?;

// rx now contains the data that was sent from tx
assert_eq!(&buf[4..], &[1, 2, 3, 4]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [`examples/`](https://github.com/Tikitikitikidesuka/ibverbs-rs/tree/main/examples) directory for complete working programs including
point-to-point channels, multi-channel scatter/gather, and distributed network barriers.

## Requirements

- Linux with RDMA-capable hardware (InfiniBand or RoCE)
- `rdma-core` development libraries (`rdma-core-devel` on RHEL/Alma, `libibverbs-dev` on Debian/Ubuntu)
- Rust 2024 edition (nightly or stable 1.85+)

A [`Dockerfile`](https://github.com/Tikitikitikidesuka/ibverbs-rs/blob/main/Dockerfile) with all dependencies pre-installed is included for convenience.

## Optional features

| Feature | Description                                         |
|---------|-----------------------------------------------------|
| `numa`  | Enables NUMA-aware thread pinning (links `libnuma`) |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

Developed by Miguel Hermoso Mantecón and Jonatan Ziegler during their respective technical studentships at CERN.

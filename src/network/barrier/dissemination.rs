use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::PeerRemoteMemoryRegion;
use crate::network::barrier::memory::{BarrierMr, PreparedBarrierMr};
use crate::network::barrier::{BarrierError, validate_peer_list};
use std::time::{Duration, Instant};

/// Returns (notify_right_idx, wait_left_idx) pairs for each round of the dissemination barrier.
///
/// In each round the distance doubles (1, 2, 4, ...). The node at `idx` notifies the peer
/// `distance` positions to the right (wrapping) and waits for the peer `distance` positions
/// to the left.
fn round_pairs(idx: usize, len: usize) -> impl Iterator<Item = (usize, usize)> {
    std::iter::successors(Some(1usize), |d| d.checked_mul(2))
        .take_while(move |&d| d < len)
        .map(move |d| ((idx + d) % len, (idx + len - d) % len))
}

/// Dissemination barrier implementation.
///
/// In each round, every node notifies a peer at exponentially increasing distance
/// and waits for a notification from the symmetric peer. O(log n) rounds with no
/// designated leader.
#[derive(Debug)]
pub struct DisseminationBarrier {
    rank: usize,
    barrier_mr: BarrierMr,
    poisoned: bool,
}

/// A [`DisseminationBarrier`] that has been allocated but not yet linked to remote peers.
#[derive(Debug)]
pub struct PreparedDisseminationBarrier {
    rank: usize,
    barrier_mr: PreparedBarrierMr,
}

impl PreparedDisseminationBarrier {
    /// Allocates a new dissemination barrier.
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedDisseminationBarrier> {
        Ok(PreparedDisseminationBarrier {
            rank,
            barrier_mr: PreparedBarrierMr::new(pd, rank, world_size)?,
        })
    }

    /// Returns this node's barrier memory region handle for exchange with peers.
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.barrier_mr.remote()
    }

    /// Links remote peer memory regions and returns a ready-to-use [`DisseminationBarrier`].
    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> DisseminationBarrier {
        DisseminationBarrier {
            rank: self.rank,
            barrier_mr: self.barrier_mr.link_remote(remote_mrs),
            poisoned: false,
        }
    }
}

impl DisseminationBarrier {
    /// Synchronizes with the given peers, blocking until all have reached the barrier or timeout.
    ///
    /// Validates that peers are sorted and unique. Self-inclusion is verified inside the
    /// synchronization itself and returns [`BarrierError::SelfNotInGroup`] if absent.
    pub fn barrier(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        validate_peer_list(peers)?;
        self.barrier_unchecked(multi_channel, peers, timeout)
    }

    /// Like [`barrier`](Self::barrier), but skips validation of the peer list.
    pub fn barrier_unchecked(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        if self.poisoned {
            return Err(BarrierError::Poisoned);
        }

        let result = self.run_barrier(multi_channel, peers, timeout);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn run_barrier(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        if peers.len() < 2 {
            return Ok(());
        }

        let start_time = Instant::now();

        let idx = peers
            .binary_search(&self.rank)
            .map_err(|_| BarrierError::SelfNotInGroup)?;

        for (right_idx, left_idx) in round_pairs(idx, peers.len()) {
            let right_rank = peers[right_idx];
            let left_rank = peers[left_idx];

            // 1. Notify the peer to the right
            self.barrier_mr.notify_peer(multi_channel, right_rank)?;

            // 2. Wait for the peer to the left
            self.barrier_mr.increase_peer_expected_epoch(left_rank);
            self.barrier_mr
                .spin_poll_peer_epoch_expected(left_rank, start_time, timeout)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_node_no_rounds() {
        let rounds: Vec<_> = round_pairs(0, 1).collect();
        assert!(rounds.is_empty());
    }

    #[test]
    fn two_nodes_one_round() {
        let rounds: Vec<_> = round_pairs(0, 2).collect();
        assert_eq!(rounds.len(), 1);
    }

    #[test]
    fn round_count_is_ceil_log2() {
        // The number of rounds should be ceil(log2(len))
        let cases = [
            (2, 1),
            (3, 2),
            (4, 2),
            (5, 3),
            (7, 3),
            (8, 3),
            (9, 4),
            (16, 4),
        ];
        for (len, expected_rounds) in cases {
            let count = round_pairs(0, len).count();
            assert_eq!(
                count, expected_rounds,
                "expected {expected_rounds} rounds for {len} nodes, got {count}"
            );
        }
    }

    #[test]
    fn two_nodes_notify_each_other() {
        // Node 0 notifies 1 and waits for 1
        let pairs_0: Vec<_> = round_pairs(0, 2).collect();
        assert_eq!(pairs_0, vec![(1, 1)]);

        // Node 1 notifies 0 and waits for 0
        let pairs_1: Vec<_> = round_pairs(1, 2).collect();
        assert_eq!(pairs_1, vec![(0, 0)]);
    }

    #[test]
    fn three_nodes_round_pairs() {
        // Node 0: round 1 (d=1) -> notify 1, wait for 2
        //         round 2 (d=2) -> notify 2, wait for 1
        let pairs: Vec<_> = round_pairs(0, 3).collect();
        assert_eq!(pairs, vec![(1, 2), (2, 1)]);
    }

    #[test]
    fn notify_and_wait_are_symmetric() {
        // If node A notifies node B in round r, then node B waits for node A in round r.
        for len in 2..=16 {
            for idx in 0..len {
                for (round, (notify_target, _)) in round_pairs(idx, len).enumerate() {
                    // Find the corresponding round for the target
                    let target_pairs: Vec<_> = round_pairs(notify_target, len).collect();
                    let (_, wait_source) = target_pairs[round];
                    assert_eq!(
                        wait_source, idx,
                        "broken symmetry: node {idx} notifies {notify_target} in round {round}, \
                         but {notify_target} waits for {wait_source} (expected {idx}) in len={len}"
                    );
                }
            }
        }
    }

    #[test]
    fn all_pairs_communicate_transitively() {
        // After all rounds, every node should be transitively connected to every other node.
        for len in 2..=16 {
            // Build direct communication graph
            let mut connected = vec![vec![false; len]; len];
            for i in 0..len {
                connected[i][i] = true;
                for (notify_target, wait_source) in round_pairs(i, len) {
                    connected[i][notify_target] = true;
                    connected[i][wait_source] = true;
                }
            }

            // Transitive closure (Floyd-Warshall)
            for k in 0..len {
                for i in 0..len {
                    for j in 0..len {
                        if connected[i][k] && connected[k][j] {
                            connected[i][j] = true;
                        }
                    }
                }
            }

            for i in 0..len {
                for j in 0..len {
                    assert!(
                        connected[i][j],
                        "nodes {i} and {j} not transitively connected in len={len}"
                    );
                }
            }
        }
    }

    #[test]
    fn indices_within_bounds() {
        for len in 1..=32 {
            for idx in 0..len {
                for (right, left) in round_pairs(idx, len) {
                    assert!(right < len, "right={right} out of bounds for len={len}");
                    assert!(left < len, "left={left} out of bounds for len={len}");
                }
            }
        }
    }

    #[test]
    fn never_notifies_self() {
        for len in 2..=16 {
            for idx in 0..len {
                for (right, left) in round_pairs(idx, len) {
                    assert_ne!(right, idx, "node {idx} notifies itself in len={len}");
                    assert_ne!(left, idx, "node {idx} waits for itself in len={len}");
                }
            }
        }
    }
}

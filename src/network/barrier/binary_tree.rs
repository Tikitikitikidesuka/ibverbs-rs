use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::PeerRemoteMemoryRegion;
use crate::network::barrier::memory::{BarrierMr, PreparedBarrierMr};
use crate::network::barrier::{BarrierError, validate_peer_list};
use std::time::{Duration, Instant};

/// Returns the parent index in a zero-indexed binary tree, or `None` for the root.
fn parent_index(idx: usize) -> Option<usize> {
    (idx > 0).then(|| (idx - 1) / 2)
}

/// Returns an iterator over the child indices that exist within a tree of `len` nodes.
fn child_indices(idx: usize, len: usize) -> impl Iterator<Item = usize> {
    [2 * idx + 1, 2 * idx + 2]
        .into_iter()
        .filter(move |&c| c < len)
}

/// Binary tree barrier implementation.
///
/// Nodes are arranged in a binary tree. The reduce phase propagates notifications
/// upward from leaves to root, then the broadcast phase propagates back down.
/// O(log n) messages.
#[derive(Debug)]
pub struct BinaryTreeBarrier {
    rank: usize,
    barrier_mr: BarrierMr,
    poisoned: bool,
}

/// A [`BinaryTreeBarrier`] that has been allocated but not yet linked to remote peers.
#[derive(Debug)]
pub struct PreparedBinaryTreeBarrier {
    rank: usize,
    barrier_mr: PreparedBarrierMr,
}

impl PreparedBinaryTreeBarrier {
    /// Allocates a new binary tree barrier.
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBinaryTreeBarrier> {
        Ok(PreparedBinaryTreeBarrier {
            rank,
            barrier_mr: PreparedBarrierMr::new(pd, rank, world_size)?,
        })
    }

    /// Returns this node's barrier memory region handle for exchange with peers.
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.barrier_mr.remote()
    }

    /// Links remote peer memory regions and returns a ready-to-use [`BinaryTreeBarrier`].
    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> BinaryTreeBarrier {
        BinaryTreeBarrier {
            rank: self.rank,
            barrier_mr: self.barrier_mr.link_remote(remote_mrs),
            poisoned: false,
        }
    }
}

impl BinaryTreeBarrier {
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

        let parent_rank = parent_index(idx).map(|pi| peers[pi]);

        let mut children_ranks_buffer = [0; 2];
        let mut count = 0;
        for ci in child_indices(idx, peers.len()) {
            children_ranks_buffer[count] = peers[ci];
            count += 1;
        }
        let children_ranks = &children_ranks_buffer[..count];

        // 1. Notify upwards
        // 1.1 Wait for children
        for &child_rank in children_ranks {
            self.barrier_mr.increase_peer_expected_epoch(child_rank);
            self.barrier_mr
                .spin_poll_peer_epoch_expected(child_rank, start_time, timeout)?;
        }
        // 1.2 Notify parent
        if let Some(parent_rank) = parent_rank {
            self.barrier_mr.notify_peer(multi_channel, parent_rank)?;
        }

        // 2. Notify downwards
        // 2.1 Wait for parent
        if let Some(parent_rank) = parent_rank {
            self.barrier_mr.increase_peer_expected_epoch(parent_rank);
            self.barrier_mr
                .spin_poll_peer_epoch_expected(parent_rank, start_time, timeout)?;
        }
        //2.2 Notify children
        self.barrier_mr
            .scatter_notify_peers(multi_channel, children_ranks)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_has_no_parent() {
        assert_eq!(parent_index(0), None);
    }

    #[test]
    fn left_child_parent() {
        // Index 1's parent is (1-1)/2 = 0
        assert_eq!(parent_index(1), Some(0));
    }

    #[test]
    fn right_child_parent() {
        // Index 2's parent is (2-1)/2 = 0
        assert_eq!(parent_index(2), Some(0));
    }

    #[test]
    fn deeper_nodes_parent() {
        // Complete tree indices:
        //        0
        //      /   \
        //     1     2
        //    / \   / \
        //   3   4 5   6
        assert_eq!(parent_index(3), Some(1));
        assert_eq!(parent_index(4), Some(1));
        assert_eq!(parent_index(5), Some(2));
        assert_eq!(parent_index(6), Some(2));
    }

    #[test]
    fn single_node_has_no_children() {
        let children: Vec<_> = child_indices(0, 1).collect();
        assert!(children.is_empty());
    }

    #[test]
    fn root_with_two_children() {
        let children: Vec<_> = child_indices(0, 3).collect();
        assert_eq!(children, vec![1, 2]);
    }

    #[test]
    fn root_with_only_left_child() {
        let children: Vec<_> = child_indices(0, 2).collect();
        assert_eq!(children, vec![1]);
    }

    #[test]
    fn leaf_has_no_children() {
        // In a 7-node tree, index 3 is a leaf (children would be 7, 8)
        let children: Vec<_> = child_indices(3, 7).collect();
        assert!(children.is_empty());
    }

    #[test]
    fn internal_node_children() {
        // In a 7-node tree, index 1 has children 3 and 4
        let children: Vec<_> = child_indices(1, 7).collect();
        assert_eq!(children, vec![3, 4]);
    }

    #[test]
    fn parent_of_child_is_self() {
        for len in 2..=16 {
            for idx in 0..len {
                for child in child_indices(idx, len) {
                    assert_eq!(
                        parent_index(child),
                        Some(idx),
                        "parent(child({idx})) != {idx} in tree of size {len}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_non_root_has_parent_within_bounds() {
        for len in 1..=16 {
            for idx in 1..len {
                let parent = parent_index(idx).expect("non-root should have parent");
                assert!(parent < idx, "parent {parent} should be < child {idx}");
                assert!(parent < len, "parent {parent} out of bounds for len {len}");
            }
        }
    }

    #[test]
    fn all_nodes_reachable_from_root() {
        for len in 1..=16 {
            let mut visited = vec![false; len];
            let mut stack = vec![0usize];
            while let Some(idx) = stack.pop() {
                visited[idx] = true;
                for child in child_indices(idx, len) {
                    stack.push(child);
                }
            }
            assert!(
                visited.iter().all(|&v| v),
                "not all nodes reachable from root in tree of size {len}"
            );
        }
    }

    #[test]
    fn tree_depth_is_logarithmic() {
        for len in 1..=64 {
            // Deepest node is the last index
            let mut depth = 0;
            let mut idx = len - 1;
            while let Some(p) = parent_index(idx) {
                idx = p;
                depth += 1;
            }
            let expected_max_depth = (len as f64).log2().floor() as usize;
            assert!(
                depth <= expected_max_depth,
                "depth {depth} exceeds expected {expected_max_depth} for len {len}"
            );
        }
    }
}

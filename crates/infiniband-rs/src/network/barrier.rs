use crate::channel::raw_channel::pending_work::MultiWorkPollError;
use crate::network::Node;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BarrierError {
    #[error("Self not in issued barrier's peers")]
    SelfNotInGroup,
    #[error("Network error: {0}")]
    NetworkError(#[from] MultiWorkPollError),
}

impl Node {
    pub fn centralized_barrier<I>(
        &mut self,
        peers: impl AsRef<[usize]>,
    ) -> Result<(), BarrierError> {
        let peers = peers.as_ref();

        if !peers.contains(&self.rank) {
            return Err(BarrierError::SelfNotInGroup);
        }

        // Contains self so it is not empty (guaranteed min)
        let coordinator = *peers.iter().min().unwrap();

        if self.rank == coordinator {
            let self_rank = self.rank;
            self.coordinator_centralized_barrier(peers.iter().copied().filter(|&p| p != self_rank))
        } else {
            self.participant_centralized_barrier(coordinator)
        }
    }

    fn coordinator_centralized_barrier(
        &mut self,
        participants: impl Iterator<Item=usize> + Clone,
    ) -> Result<(), BarrierError> {
        // Wait for all participants
        self.gather_immediate(participants.clone())?
            .iter()
            .all(|wc| wc.immediate_data() == Some(Self::PARTICIPANT_READY));

        // Notify all participants
        self.multicast_with_immediate(participants, &[], Self::COORDINATOR_READY)?;

        Ok(())
    }

    fn participant_centralized_barrier(&self, coordinator: usize) -> Result<(), BarrierError> {
        todo!()
        // Notify coordinator
        //self.send_immediate(coordinator, Self::PARTICIPANT_READY);
        /// :( -> This only works if specific channel for this like Alberto did
        /// or back to the memory write read method from my previous implementation

        // Wait for coordinator
    }

    const PARTICIPANT_READY: u32 = 432982347;
    const COORDINATOR_READY: u32 = 958729371;
}

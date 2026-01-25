use crate::channel::multi_channel::MultiChannel;

impl MultiChannel {
    pub fn centralized_barrier<I>(&mut self, peers: I)
    where
        I: IntoIterator<Item = usize>,
        I::IntoIter: ExactSizeIterator,
    {
        // If no coordinator means zero peers in the barrier (finished)
        if let Some(coordinator) = peers.into_iter().min() {
            //if self.id
            // Frik... this is necessarily a network node operation...
            // Node needs a rank
        }
    }
}

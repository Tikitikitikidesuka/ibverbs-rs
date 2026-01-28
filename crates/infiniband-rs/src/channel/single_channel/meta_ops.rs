use crate::channel::single_channel::SingleChannel;

impl SingleChannel {
    /*
    pub fn sync_epoch(&mut self) -> WorkSpinPollResult {
        self.meta_mr.increase_sync_epoch();
        let wr = self.meta_mr.prepare_sync_epoch_wr();
        self.channel.write(wr)
    }

    pub fn get_sync_epoch(&mut self) -> usize {
        self.meta_mr.get_sync_epoch()
    }
    */
}

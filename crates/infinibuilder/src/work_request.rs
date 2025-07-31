use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::rc::Rc;
use ibverbs::{ibv_wc, CompletionQueue};

#[must_use = "Work request should be polled until complete or waited for"]
pub struct WorkRequest {
    pub(crate) id: u64,
    pub(crate) cq: Rc<CompletionQueue>,
    pub(crate) wc_cache: Rc<RefCell<HashMap<u64, ibv_wc>>>,
    pub(crate) dead_wr: Rc<RefCell<HashSet<u64>>>,
}

impl WorkRequest {
    fn gather_completions(&self) -> io::Result<()> {
        const CQ_POLL_ARR_SIZE: usize = 16;
        let mut cq_poll_arr = [ibv_wc::default(); CQ_POLL_ARR_SIZE];

        // Get new completions
        let mut completions = self.cq.poll(&mut cq_poll_arr[..])?;
        while completions.len() != 0 {
            completions.into_iter().for_each(|completion| {
                // Insert it to the completion cache only if it is not a dead request
                if !self.dead_wr.borrow_mut().remove(&completion.wr_id()) {
                    self.wc_cache
                        .borrow_mut()
                        .insert(completion.wr_id(), *completion);
                }
            });
            completions = self.cq.poll(&mut cq_poll_arr[..])?;
        }

        Ok(())
    }

    pub fn poll(&self) -> io::Result<bool> {
        self.gather_completions()?;
        Ok(self.wc_cache.borrow().contains_key(&self.id))
    }

    pub fn wait(self) -> io::Result<()> {
        while !self.poll()? {
            std::hint::spin_loop();
        }

        Ok(())
    }
}

impl Drop for WorkRequest {
    fn drop(&mut self) {
        // If already completed, remove it
        if let None = self.wc_cache.borrow_mut().remove(&self.id) {
            println!("Request {:?} ignored... Forgetting it...", self.id);
            // If not completed, add it to the dead wr set
            self.dead_wr.borrow_mut().insert(self.id);
            // It must be removed later when inserted
        }
    }
}

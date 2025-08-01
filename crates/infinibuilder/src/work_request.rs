use crate::WorkRequestStatus::{Done, Waiting};
use ibverbs::{CompletionQueue, ibv_wc};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::rc::Rc;

#[must_use = "Work request should be polled until complete or waited for"]
pub struct WorkRequest {
    pub(crate) id: u64,
    pub(crate) cq: Rc<CompletionQueue>,
    pub(crate) wc_cache: Rc<RefCell<HashMap<u64, ibv_wc>>>,
    pub(crate) dead_wr: Rc<RefCell<HashSet<u64>>>,
}

pub enum WorkRequestStatus {
    Done(ibv_wc),
    Waiting,
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

    pub fn poll(&self) -> io::Result<WorkRequestStatus> {
        self.gather_completions()?;
        match self.wc_cache.borrow().get(&self.id) {
            Some(wc) => Ok(Done(*wc)),
            None => Ok(Waiting),
        }
    }

    pub fn wait(self) -> io::Result<ibv_wc> {
        loop {
            match self.poll()? {
                Done(wc) => return Ok(wc),
                Waiting => std::hint::spin_loop(),
            }
        }
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

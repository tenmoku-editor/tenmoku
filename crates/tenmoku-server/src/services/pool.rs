//! The fs service's workers: a few threads per session, started as requests
//! need them, so a slow disk holds up only the requests on its workers and
//! a burst of listings doesn't start a thread each.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

pub(crate) type Job = Box<dyn FnOnce() + Send>;

pub(crate) struct Pool {
    shared: Arc<Shared>,
}

struct Shared {
    name: &'static str,
    most: usize,
    state: Mutex<State>,
    work: Condvar,
}

#[derive(Default)]
struct State {
    jobs: VecDeque<Job>,
    threads: usize,
    idle: usize,
    closed: bool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Pool {
    /// A pool of at most `most` threads named `name`.
    pub(crate) fn new(name: &'static str, most: usize) -> Pool {
        Pool {
            shared: Arc::new(Shared {
                name,
                most,
                state: Mutex::default(),
                work: Condvar::new(),
            }),
        }
    }

    /// Queues `job` for a worker, starting one if none is idle and there is
    /// room. Hands `job` back if there is no worker and none can start.
    pub(crate) fn run(&self, job: Job) -> Result<(), Job> {
        let mut st = self.shared.lock();
        // An idle worker woken for a queued job is still counted idle
        // until it takes it.
        if st.idle <= st.jobs.len() && st.threads < self.shared.most {
            let shared = self.shared.clone();
            let started = thread::Builder::new()
                .name(self.shared.name.into())
                .spawn(move || work(&shared));
            match started {
                Ok(_) => st.threads += 1,
                Err(e) if st.threads == 0 => {
                    log!("can't start a {} worker: {e}", self.shared.name);
                    return Err(job);
                }
                // The workers there are will get to it.
                Err(_) => {}
            }
        }
        st.jobs.push_back(job);
        self.shared.work.notify_one();
        Ok(())
    }
}

impl Drop for Pool {
    /// Stops the workers once their jobs in hand are done. Jobs still
    /// queued are dropped: their link is gone, so no one waits for them.
    fn drop(&mut self) {
        let queued = {
            let mut st = self.shared.lock();
            st.closed = true;
            std::mem::take(&mut st.jobs)
        };
        self.shared.work.notify_all();
        drop(queued);
    }
}

fn work(shared: &Shared) {
    /// Counts the worker out however it ends, a job's panic included.
    struct Leaving<'a>(&'a Shared);
    impl Drop for Leaving<'_> {
        fn drop(&mut self) {
            self.0.lock().threads -= 1;
        }
    }
    let _leaving = Leaving(shared);
    loop {
        let job = {
            let mut st = shared.lock();
            loop {
                if st.closed {
                    return;
                }
                if let Some(job) = st.jobs.pop_front() {
                    break job;
                }
                st.idle += 1;
                st = shared.work.wait(st).unwrap_or_else(|e| e.into_inner());
                st.idle -= 1;
            }
        };
        job();
    }
}

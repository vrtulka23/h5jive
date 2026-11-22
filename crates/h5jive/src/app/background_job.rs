//! Keyed worker jobs with stale-result cancellation for views and plugins.
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, TryRecvError},
    Arc,
};

pub(super) struct BackgroundJob<K, T> {
    active: Option<(K, Receiver<Result<T, String>>)>,
    generation: Arc<AtomicU64>,
}

impl<K, T> Default for BackgroundJob<K, T> {
    fn default() -> Self {
        Self {
            active: None,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl<K: Send + 'static, T: Send + 'static> BackgroundJob<K, T> {
    pub(super) fn start(
        &mut self,
        key: K,
        worker: impl FnOnce(&dyn Fn() -> bool) -> Result<T, String> + Send + 'static,
    ) {
        self.cancel();
        let generation = Arc::clone(&self.generation);
        let expected = generation.load(Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let cancelled = || generation.load(Ordering::Relaxed) != expected;
            let result = worker(&cancelled);
            if !cancelled() {
                let _ = sender.send(result);
            }
        });
        self.active = Some((key, receiver));
    }

    pub(super) fn cancel(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.active = None;
    }

    pub(super) fn is_busy(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn key(&self) -> Option<&K> {
        self.active.as_ref().map(|(key, _)| key)
    }

    pub(super) fn poll(&mut self, disconnected: &str) -> Option<(K, Result<T, String>)> {
        let (_, receiver) = self.active.as_ref()?;
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(disconnected.to_owned()),
        };
        let (key, _) = self.active.take().expect("An active job was just polled");
        Some((key, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_a_job_discards_its_stale_result() {
        let mut jobs = BackgroundJob::<usize, usize>::default();
        jobs.start(1, |_| Ok(10));
        jobs.start(2, |_| Ok(20));
        for _ in 0..1000 {
            if let Some((key, result)) = jobs.poll("worker stopped") {
                assert_eq!(key, 2);
                assert_eq!(result.unwrap(), 20);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("worker did not finish");
    }
}

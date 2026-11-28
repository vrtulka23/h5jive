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

pub(super) enum JobEvent<T, P> {
    Progress(P),
    Finished(Result<T, String>),
}

/// A keyed worker that can publish intermediate previews before its final result.
pub(super) struct ProgressJob<K, T, P> {
    active: Option<(K, Receiver<JobEvent<T, P>>)>,
    generation: Arc<AtomicU64>,
}

impl<K, T, P> Default for ProgressJob<K, T, P> {
    fn default() -> Self {
        Self {
            active: None,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl<K: Send + Clone + 'static, T: Send + 'static, P: Send + 'static> ProgressJob<K, T, P> {
    pub(super) fn start(
        &mut self,
        key: K,
        worker: impl FnOnce(&(dyn Fn() -> bool + Sync), &(dyn Fn(P) -> bool + Sync)) -> Result<T, String>
            + Send
            + 'static,
    ) {
        self.cancel();
        let generation = Arc::clone(&self.generation);
        let expected = generation.load(Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let cancelled = || generation.load(Ordering::Relaxed) != expected;
            let progress_sender = sender.clone();
            let progress = move |value| {
                !cancelled() && progress_sender.send(JobEvent::Progress(value)).is_ok()
            };
            let result = worker(&cancelled, &progress);
            drop(progress);
            if !cancelled() {
                let _ = sender.send(JobEvent::Finished(result));
            }
        });
        self.active = Some((key, receiver));
    }

    pub(super) fn cancel(&mut self) -> bool {
        let was_active = self.active.is_some();
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.active = None;
        was_active
    }

    pub(super) fn is_busy(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn key(&self) -> Option<&K> {
        self.active.as_ref().map(|(key, _)| key)
    }

    pub(super) fn poll(&mut self, disconnected: &str) -> Option<(K, JobEvent<T, P>)> {
        let (key, receiver) = self.active.as_ref()?;
        let event = match receiver.try_recv() {
            Ok(JobEvent::Progress(value)) => return Some((key.clone(), JobEvent::Progress(value))),
            Ok(JobEvent::Finished(result)) => JobEvent::Finished(result),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => JobEvent::Finished(Err(disconnected.to_owned())),
        };
        let (key, _) = self.active.take().expect("An active job was just polled");
        Some((key, event))
    }
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
        worker: impl FnOnce(&(dyn Fn() -> bool + Sync)) -> Result<T, String> + Send + 'static,
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

    pub(super) fn cancel(&mut self) -> bool {
        let was_active = self.active.is_some();
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.active = None;
        was_active
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

    /// Poll a keyed result job and store its outcome in the view's cache.
    pub(super) fn finish_into(
        &mut self,
        cache: &mut Option<(K, Result<T, String>)>,
        disconnected: &str,
    ) -> Option<String>
    where
        K: Clone,
    {
        let (key, result) = self.poll(disconnected)?;
        let notice = result.as_ref().err().cloned();
        *cache = Some((key, result));
        notice
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

    #[test]
    fn progress_is_published_before_the_keyed_result() {
        let mut jobs = ProgressJob::<usize, usize, usize>::default();
        jobs.start(7, |_, progress| {
            progress(3);
            Ok(5)
        });
        let mut saw_progress = false;
        for _ in 0..1000 {
            match jobs.poll("worker stopped") {
                Some((key, JobEvent::Progress(value))) => {
                    assert_eq!(key, 7);
                    assert_eq!(value, 3);
                    saw_progress = true;
                }
                Some((key, JobEvent::Finished(result))) => {
                    assert_eq!(key, 7);
                    assert_eq!(result.unwrap(), 5);
                    assert!(saw_progress);
                    return;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        }
        panic!("worker did not finish");
    }
}

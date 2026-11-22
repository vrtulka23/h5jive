use std::sync::mpsc::{self, Receiver};

/// The common poll outcome for background work. The caller decides how to display errors.
pub(super) enum JobPoll<T> {
    Pending,
    Ready(T),
    Disconnected,
}

pub(super) fn poll_job<T>(receiver: &Receiver<T>) -> JobPoll<T> {
    match receiver.try_recv() {
        Ok(value) => JobPoll::Ready(value),
        Err(mpsc::TryRecvError::Empty) => JobPoll::Pending,
        Err(mpsc::TryRecvError::Disconnected) => JobPoll::Disconnected,
    }
}

pub(super) fn finish_keyed_result_job<K: Clone, T>(
    job: &mut Option<(K, Receiver<Result<T, String>>)>,
    cache: &mut Option<(K, Result<T, String>)>,
    stopped_message: &'static str,
) {
    match job
        .as_ref()
        .map(|(key, receiver)| (key.clone(), poll_job(receiver)))
    {
        Some((key, JobPoll::Ready(result))) => {
            *cache = Some((key, result));
            *job = None;
        }
        Some((key, JobPoll::Disconnected)) => {
            *cache = Some((key, Err(stopped_message.into())));
            *job = None;
        }
        Some((_, JobPoll::Pending)) | None => {}
    }
}

/// Shared draft/apply operation; view-specific validation and cache updates stay with the view.
pub(super) struct DraftPair<'a, T> {
    draft: &'a T,
    applied: &'a mut T,
}

impl<'a, T: Clone + PartialEq> DraftPair<'a, T> {
    pub(super) fn new(draft: &'a T, applied: &'a mut T) -> Self {
        Self { draft, applied }
    }

    pub(super) fn pending(&self) -> bool {
        self.draft != self.applied
    }

    pub(super) fn apply(&mut self) -> bool {
        let changed = self.pending();
        *self.applied = self.draft.clone();
        changed
    }
}

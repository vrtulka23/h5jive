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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PinnedDefault {
    Start,
    Middle,
    End,
}
impl PinnedDefault {
    pub(crate) fn from_key(key: &str) -> Option<Self> {
        match key {
            "start" => Some(Self::Start),
            "middle" => Some(Self::Middle),
            "end" => Some(Self::End),
            _ => None,
        }
    }
    pub(crate) fn index(self, length: usize) -> usize {
        match self {
            Self::Start => 0,
            Self::Middle => length / 2,
            Self::End => length.saturating_sub(1),
        }
    }
}

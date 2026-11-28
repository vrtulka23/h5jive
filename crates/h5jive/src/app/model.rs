//! Draft widget state and view-specific controls kept on the GUI side.
use super::*;

const INFO_NOTICE_DURATION: Duration = Duration::from_secs(5);
const SUCCESS_NOTICE_DURATION: Duration = Duration::from_secs(4);
const WARNING_NOTICE_DURATION: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoticeKind {
    Loading,
    Info,
    Success,
    Warning,
    Error,
}

impl NoticeKind {
    fn duration(self) -> Option<Duration> {
        match self {
            Self::Loading | Self::Error => None,
            Self::Info => Some(INFO_NOTICE_DURATION),
            Self::Success => Some(SUCCESS_NOTICE_DURATION),
            Self::Warning => Some(WARNING_NOTICE_DURATION),
        }
    }
}

pub(crate) struct AppNotice {
    id: u64,
    pub(crate) kind: NoticeKind,
    pub(crate) message: String,
    expires_at: Option<Instant>,
}

impl AppNotice {
    fn new(id: u64, kind: NoticeKind, message: String) -> Self {
        Self::started_at(id, kind, message, Instant::now())
    }

    pub(crate) fn started_at(
        id: u64,
        kind: NoticeKind,
        message: String,
        started_at: Instant,
    ) -> Self {
        Self {
            id,
            kind,
            message,
            expires_at: kind.duration().map(|duration| started_at + duration),
        }
    }

    pub(crate) fn is_expired(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|expires_at| now >= expires_at)
    }

    pub(crate) fn expires_at(&self) -> Option<Instant> {
        self.expires_at
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

#[derive(Default)]
pub(crate) struct NoticeCenter {
    notices: Vec<AppNotice>,
    next_id: u64,
}

impl NoticeCenter {
    pub(crate) fn show(&mut self, kind: NoticeKind, message: impl Into<String>) {
        debug_assert_ne!(kind, NoticeKind::Loading);
        let message = message.into();
        let started_at = Instant::now();
        if let Some(existing) = self
            .notices
            .iter_mut()
            .find(|notice| notice.kind == kind && notice.message == message)
        {
            existing.expires_at = kind.duration().map(|duration| started_at + duration);
            return;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.notices.push(AppNotice::new(id, kind, message));
    }

    pub(crate) fn info(&mut self, message: impl Into<String>) {
        self.show(NoticeKind::Info, message);
    }

    pub(crate) fn success(&mut self, message: impl Into<String>) {
        self.show(NoticeKind::Success, message);
    }

    pub(crate) fn warning(&mut self, message: impl Into<String>) {
        self.show(NoticeKind::Warning, message);
    }

    pub(crate) fn error(&mut self, message: impl Into<String>) {
        self.show(NoticeKind::Error, message);
    }

    pub(crate) fn expire(&mut self, now: Instant) {
        self.notices.retain(|notice| !notice.is_expired(now));
    }

    pub(crate) fn dismiss(&mut self, id: u64) {
        self.notices.retain(|notice| notice.id() != id);
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.notices.iter().filter_map(AppNotice::expires_at).min()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &AppNotice> {
        self.notices.iter()
    }
}

impl App {
    pub(crate) fn show_notice(&mut self, kind: NoticeKind, message: impl Into<String>) {
        self.session.notices.show(kind, message);
    }

    pub(crate) fn expire_notices(&mut self, now: Instant) {
        self.session.notices.expire(now);
    }

    pub(crate) fn dismiss_notice(&mut self, id: u64) {
        self.session.notices.dismiss(id);
    }

    pub(crate) fn next_notice_deadline(&self) -> Option<Instant> {
        self.session.notices.next_deadline()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CellKey {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) coordinates: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingCellEdit {
    pub(crate) original: String,
    pub(crate) replacement: String,
}

pub(crate) struct CellEditor {
    pub(crate) key: CellKey,
    pub(crate) original: String,
    pub(crate) text: String,
    pub(crate) focus: bool,
}

pub(crate) struct CellEditJob {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) restore_selection: Option<Selection>,
    pub(crate) keys: Vec<CellKey>,
    pub(crate) job: BackgroundJob<(), (h5jive_data::NumericApplyOutcome, Result<File, String>)>,
}

pub(crate) struct FindReplaceDialog {
    pub(crate) selection: Selection,
    pub(crate) open_requested: bool,
    pub(crate) needle: String,
    pub(crate) replacement: String,
    pub(crate) match_mode: usize,
    pub(crate) tolerance: String,
    pub(crate) decimals: i32,
    pub(crate) first_result: u64,
    pub(crate) summary: Option<Result<h5jive_data::NumericSearchSummary, String>>,
    pub(crate) progress: Arc<AtomicU64>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) search_job: BackgroundJob<(), h5jive_data::NumericSearchSummary>,
    pub(crate) message: Option<String>,
    pub(crate) preserved: bool,
}

pub(crate) struct BulkReplaceJob {
    pub(crate) selection: Selection,
    pub(crate) progress: Arc<AtomicU64>,
    pub(crate) job: BackgroundJob<(), (h5jive_data::NumericReplaceOutcome, Result<File, String>)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GeoViewOptions {
    pub(crate) time: Option<usize>,
    pub(crate) pinned: usize,
    pub(crate) pixel_style: usize,
    pub(crate) scan_filter: ScanFilter,
    pub(crate) dummy_color: bool,
    pub(crate) dummy_rgb: [f32; 3],
    pub(crate) opacity: f32,
    pub(crate) show_pics: bool,
}

impl GeoViewOptions {
    fn new(show_pics: bool) -> Self {
        Self {
            time: None,
            pinned: 0,
            pixel_style: 2,
            scan_filter: ScanFilter::Both,
            dummy_color: false,
            dummy_rgb: [0.647, 0.282, 0.039],
            opacity: 0.5,
            show_pics,
        }
    }
}

pub(crate) struct GeoViewControls {
    pub(crate) draft: GeoViewOptions,
    pub(crate) applied: GeoViewOptions,
    source: Option<(usize, String)>,
}

impl GeoViewControls {
    pub(crate) fn new(show_pics: bool) -> Self {
        let options = GeoViewOptions::new(show_pics);
        Self {
            draft: options,
            applied: options,
            source: None,
        }
    }

    pub(crate) fn with_dummy_rgb(mut self, accent: [f32; 4]) -> Self {
        let rgb = [accent[0], accent[1], accent[2]];
        self.draft.dummy_rgb = rgb;
        self.applied.dummy_rgb = rgb;
        self
    }

    pub(crate) fn sync_source(
        &mut self,
        source: Option<(&Selection, &[usize])>,
        default: PinnedDefault,
        scan_default: ScanFilter,
    ) {
        let key = source.map(|(selection, _)| (selection.file, selection.path.clone()));
        if self.source != key {
            self.source = key;
            let pinned = source
                .filter(|(_, shape)| shape.len() == 4)
                .map_or(0, |(_, shape)| default.index(shape[3]));
            self.draft.time = None;
            self.applied.time = None;
            self.draft.pinned = pinned;
            self.applied.pinned = pinned;
            self.draft.scan_filter = scan_default;
            self.applied.scan_filter = scan_default;
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VolumeProjection {
    Isometric,
    Xy,
    Yz,
    Zx,
}

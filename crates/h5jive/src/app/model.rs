//! Draft widget state and view-specific controls kept on the GUI side.
use super::*;

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
    pub(crate) receiver: mpsc::Receiver<(h5jive_data::NumericApplyOutcome, Result<File, String>)>,
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
    pub(crate) search_job:
        Option<mpsc::Receiver<Result<h5jive_data::NumericSearchSummary, String>>>,
    pub(crate) message: Option<String>,
    pub(crate) preserved: bool,
}

pub(crate) struct BulkReplaceJob {
    pub(crate) selection: Selection,
    pub(crate) progress: Arc<AtomicU64>,
    pub(crate) receiver: mpsc::Receiver<(h5jive_data::NumericReplaceOutcome, Result<File, String>)>,
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

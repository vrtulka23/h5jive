//! Draft widget state and view-specific controls kept on the GUI side.
use super::*;

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

//! Data shapes, slices, cache keys, and loaded values shared by the views.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MapKey {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) time: Option<usize>,
    pub(crate) spectral: usize,
}

pub(crate) struct MapPoint {
    pub(crate) latitude: f32,
    pub(crate) longitude: f32,
    pub(crate) value: f64,
    pub(crate) special: Option<SpecialValue>,
    pub(crate) scan_direction: Option<ScanDirectionClass>,
    pub(crate) footprint: Option<[[f32; 2]; 4]>, // longitude, latitude corners
}
pub(crate) struct MapData {
    pub(crate) points: Vec<MapPoint>,
    pub(crate) range: Option<(f64, f64)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MapViewport {
    pub(crate) center: [f32; 2], // longitude, latitude
    pub(crate) zoom: f32,
}
impl Default for MapViewport {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            zoom: 1.0,
        }
    }
}
impl MapViewport {
    pub(crate) fn project(self, origin: [f32; 2], size: [f32; 2], lon: f32, lat: f32) -> [f32; 2] {
        [
            origin[0] + size[0] * (0.5 + (lon - self.center[0]) * self.zoom / 360.0),
            origin[1] + size[1] * (0.5 - (lat - self.center[1]) * self.zoom / 180.0),
        ]
    }
    pub(crate) fn unproject(self, origin: [f32; 2], size: [f32; 2], point: [f32; 2]) -> [f32; 2] {
        [
            self.center[0] + ((point[0] - origin[0]) / size[0] - 0.5) * 360.0 / self.zoom,
            self.center[1] - ((point[1] - origin[1]) / size[1] - 0.5) * 180.0 / self.zoom,
        ]
    }
    pub(crate) fn clamp(&mut self) {
        let half_lon = 180.0 / self.zoom;
        let half_lat = 90.0 / self.zoom;
        self.center[0] = self.center[0].clamp(-180.0 + half_lon, 180.0 - half_lon);
        self.center[1] = self.center[1].clamp(-90.0 + half_lat, 90.0 - half_lat);
    }
    pub(crate) fn zoom_at(
        &mut self,
        origin: [f32; 2],
        size: [f32; 2],
        mouse: [f32; 2],
        factor: f32,
        max_zoom: f32,
    ) {
        let anchor = self.unproject(origin, size, mouse);
        self.zoom = (self.zoom * factor).clamp(1.0, max_zoom);
        self.center[0] = anchor[0] - ((mouse[0] - origin[0]) / size[0] - 0.5) * 360.0 / self.zoom;
        self.center[1] = anchor[1] + ((mouse[1] - origin[1]) / size[1] - 0.5) * 180.0 / self.zoom;
        self.clamp();
    }
    pub(crate) fn pan(&mut self, size: [f32; 2], delta: [f32; 2]) {
        self.center[0] -= delta[0] * 360.0 / (size[0] * self.zoom);
        self.center[1] += delta[1] * 180.0 / (size[1] * self.zoom);
        self.clamp();
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Selection {
    pub(crate) file: usize,
    pub(crate) path: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
/// A table or heatmap plane chosen from an arbitrary-rank dataset.
pub(crate) struct TableSlice {
    pub(crate) row_axis: Option<usize>,
    pub(crate) column_axis: usize,
    pub(crate) pinned: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProfileSlice {
    pub(crate) x_axis: usize,
    pub(crate) y_axis: Option<usize>,
    pub(crate) pinned: Vec<usize>,
    pub(crate) y_indexes: String,
}
impl ProfileSlice {
    pub(crate) fn new(shape: &[usize], default: PinnedDefault) -> Self {
        let pinned = pinned_indices(shape, default);
        let y_axis = (shape.len() > 1).then_some(1);
        let y_indexes = y_axis.map_or_else(String::new, |axis| pinned[axis].to_string());
        Self {
            x_axis: 0,
            y_axis,
            pinned,
            y_indexes,
        }
    }
}
pub(crate) fn parse_profile_indexes(text: &str, length: usize) -> Result<Vec<usize>, String> {
    if text.trim().is_empty() {
        return Err("Enter one or more Y indexes, separated by commas.".into());
    }
    let mut indexes = Vec::new();
    for part in text.split(',') {
        let index: usize = part
            .trim()
            .parse()
            .map_err(|_| format!("Invalid Y index: {}", part.trim()))?;
        if index >= length {
            return Err(format!(
                "Y index {index} exceeds the last index {}",
                length.saturating_sub(1)
            ));
        }
        if !indexes.contains(&index) {
            indexes.push(index);
        }
        if indexes.len() > 16 {
            return Err("Select at most 16 Y indexes.".into());
        }
    }
    indexes.sort_unstable();
    Ok(indexes)
}
#[derive(Clone)]
pub(crate) struct ProfileBin {
    pub(crate) first: f64,
    pub(crate) last: f64,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) count: usize,
}
impl Default for ProfileBin {
    fn default() -> Self {
        Self {
            first: 0.0,
            last: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            count: 0,
        }
    }
}
pub(crate) struct ProfileLine {
    pub(crate) index: usize,
    pub(crate) bins: Vec<ProfileBin>,
}
pub(crate) struct ProfileData {
    pub(crate) x_len: usize,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) lines: Vec<ProfileLine>,
}
/// Aggregate a profile into bins while preserving first, last, and extrema.
pub(crate) fn load_profile(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &ProfileSlice,
    visibility: &ValueVisibility,
) -> Result<ProfileData, String> {
    if shape.iter().any(|&length| length == 0) {
        return Err("Empty dataset".into());
    }
    let x_len = shape[slice.x_axis];
    let bin_count = x_len.min(4096);
    let indexes = slice.y_axis.map_or_else(
        || Ok(vec![0]),
        |axis| parse_profile_indexes(&slice.y_indexes, shape[axis]),
    )?;
    let mut lines = Vec::new();
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for index in indexes {
        let mut bins = vec![ProfileBin::default(); bin_count];
        for first_x in (0..x_len).step_by(65_536) {
            let length = (x_len - first_x).min(65_536);
            let mut varying = vec![(slice.x_axis, first_x, length)];
            if let Some(axis) = slice.y_axis {
                varying.push((axis, index, 1));
            }
            let (start, count) = region_from_pins(&slice.pinned, &varying);
            let region = file
                .region(path, &start, &count)
                .map_err(|err| err.to_string())?;
            for (offset, &value) in region.data.iter().enumerate() {
                if finite_value(value, region.fill_value).is_none()
                    || !visibility.allows(value, region.fill_value)
                {
                    continue;
                }
                let x = first_x + offset;
                let bin_index = (x as u128 * bin_count as u128 / x_len as u128) as usize;
                let bin = &mut bins[bin_index];
                if bin.count == 0 {
                    bin.first = value;
                }
                bin.last = value;
                bin.min = bin.min.min(value);
                bin.max = bin.max.max(value);
                bin.count += 1;
                minimum = minimum.min(value);
                maximum = maximum.max(value);
            }
        }
        lines.push(ProfileLine { index, bins });
    }
    Ok(ProfileData {
        x_len,
        min: minimum,
        max: maximum,
        lines,
    })
}
impl TableSlice {
    pub(crate) fn new(shape: &[usize], pinned_default: PinnedDefault) -> Self {
        Self {
            row_axis: (shape.len() > 1).then_some(0),
            column_axis: usize::from(shape.len() > 1),
            pinned: pinned_indices(shape, pinned_default),
        }
    }
    pub(crate) fn with_leading_time(
        shape: &[usize],
        first_is_time: bool,
        pinned_default: PinnedDefault,
    ) -> Self {
        let mut slice = Self::new(shape, pinned_default);
        if first_is_time && shape.len() >= 2 {
            slice.row_axis = (shape.len() >= 3).then_some(1);
            slice.column_axis = if shape.len() >= 3 { 2 } else { 1 };
        }
        slice
    }
    pub(crate) fn rows(&self, shape: &[usize]) -> usize {
        self.row_axis
            .map_or(shape[self.column_axis], |axis| shape[axis])
    }
    pub(crate) fn columns(&self, shape: &[usize]) -> usize {
        self.row_axis.map_or(1, |_| shape[self.column_axis])
    }
    pub(crate) fn region(
        &self,
        shape: &[usize],
        row_start: usize,
        row_end: usize,
        col_start: usize,
        col_end: usize,
    ) -> (Vec<usize>, Vec<usize>) {
        debug_assert_eq!(shape.len(), self.pinned.len());
        if let Some(axis) = self.row_axis {
            region_from_pins(
                &self.pinned,
                &[
                    (axis, row_start, row_end - row_start),
                    (self.column_axis, col_start, col_end - col_start),
                ],
            )
        } else {
            region_from_pins(
                &self.pinned,
                &[(self.column_axis, row_start, row_end - row_start)],
            )
        }
    }
    pub(crate) fn region_offset(
        &self,
        count: &[usize],
        row: usize,
        column: usize,
        start: &[usize],
    ) -> usize {
        let mut coords = self.pinned.clone();
        if let Some(axis) = self.row_axis {
            coords[axis] = row;
            coords[self.column_axis] = column;
        } else {
            coords[self.column_axis] = row;
        }
        coords
            .iter()
            .zip(start)
            .zip(count)
            .fold(0, |offset, ((&index, &first), &length)| {
                offset * length + index - first
            })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegionKey {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) start: Vec<usize>,
    pub(crate) count: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeatmapKey {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) slice: TableSlice,
    pub(crate) bounds: [usize; 4],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeatmapRangeKey {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) slice: TableSlice,
}
pub(crate) struct HeatmapImage {
    pub(crate) bounds: [usize; 4],
    pub(crate) grid: [usize; 2],
    pub(crate) values: Vec<f64>,
    pub(crate) plotted: Vec<f64>,
    pub(crate) fill_value: Option<f64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeatmapOverviewKey {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) slice: TableSlice,
    pub(crate) grid: [usize; 2],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VolumeSlice {
    pub(crate) axes: [Option<usize>; 3],
    pub(crate) hidden: Option<usize>,
    pub(crate) pinned: usize,
    pub(crate) ranges: Vec<[usize; 2]>,
}
impl VolumeSlice {
    pub(crate) fn new(shape: &[usize], first_is_time: bool, default: PinnedDefault) -> Self {
        let hidden = if shape.len() == 4 {
            Some(if first_is_time { 0 } else { 3 })
        } else {
            None
        };
        let mut visible = (0..shape.len()).filter(|axis| Some(*axis) != hidden);
        Self {
            axes: [visible.next(), visible.next(), visible.next()],
            hidden,
            pinned: hidden.map_or(0, |axis| default.index(shape[axis])),
            ranges: shape
                .iter()
                .map(|&length| [0, length.saturating_sub(1)])
                .collect(),
        }
    }
    pub(crate) fn visible_count(&self) -> usize {
        self.axes.iter().flatten().count()
    }
    pub(crate) fn set_hidden(&mut self, enabled: bool, default: PinnedDefault, shape: &[usize]) {
        if enabled && self.hidden.is_none() && self.visible_count() > 1 {
            let slot = self.visible_count() - 1;
            let axis = self.axes[slot].take().unwrap();
            self.hidden = Some(axis);
            self.pinned = default.index(shape[axis]);
        } else if !enabled {
            if let Some(axis) = self.hidden.take() {
                self.axes[self.visible_count()] = Some(axis);
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VolumeKey {
    pub(crate) file: usize,
    pub(crate) path: String,
    pub(crate) slice: VolumeSlice,
}
pub(crate) struct ExactVolumeData {
    pub(crate) values: Vec<f32>,
    pub(crate) specials: Vec<u8>,
    pub(crate) dimensions: [usize; 3],
    pub(crate) strides: [usize; 3],
    pub(crate) bounds: [[f32; 2]; 3],
    pub(crate) color_range: Option<(f64, f64)>,
}

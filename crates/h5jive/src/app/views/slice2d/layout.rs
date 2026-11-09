use super::super::super::axis_ticks;

pub(crate) fn heatmap_layout(
    rows: usize,
    columns: usize,
    available: [f32; 2],
    min_cell: f32,
    max_bins: usize,
) -> ([f32; 2], [usize; 2]) {
    let canvas = [(available[0] - 2.0).max(1.0), (available[1] - 2.0).max(1.0)];
    let mut bins = [
        columns.min(((canvas[0] / min_cell).floor() as usize).max(1)),
        rows.min(((canvas[1] / min_cell).floor() as usize).max(1)),
    ];
    if bins[0].saturating_mul(bins[1]) > max_bins {
        let factor = (max_bins as f64 / bins[0] as f64 / bins[1] as f64).sqrt();
        bins[0] = ((bins[0] as f64 * factor).floor() as usize).max(1);
        bins[1] = ((bins[1] as f64 * factor).floor() as usize).max(1);
    }
    (canvas, bins)
}

pub(crate) fn heatmap_axis_ticks(start: usize, end: usize, maximum: usize) -> Vec<usize> {
    if end <= start {
        return Vec::new();
    }
    axis_ticks(end - start, maximum)
        .into_iter()
        .map(|index| start + index)
        .collect()
}

pub(crate) fn heatmap_zoom_limits(current: [f64; 4], full: [f64; 4], factor: f64) -> [f64; 4] {
    let axis = |low: f64, high: f64, full_low: f64, full_high: f64| {
        let span = ((high - low) / factor).clamp(1.0, full_high - full_low);
        let center = ((low + high) * 0.5).clamp(full_low + span * 0.5, full_high - span * 0.5);
        [center - span * 0.5, center + span * 0.5]
    };
    let x = axis(current[0], current[1], full[0], full[1]);
    let y = axis(current[2], current[3], full[2], full[3]);
    [x[0], x[1], y[0], y[1]]
}

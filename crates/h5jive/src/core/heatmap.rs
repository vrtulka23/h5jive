//! Bounded reads and sampling for the two-dimensional plot.
use super::*;

pub(crate) fn heatmap_visible_bounds(rows: usize, columns: usize, limits: [f64; 4]) -> [usize; 4] {
    let clamp = |v: f64, length: usize| -> usize { v.floor().clamp(0.0, length as f64) as usize };
    [
        clamp(rows as f64 - limits[3], rows),
        limits[2]
            .is_finite()
            .then(|| (rows as f64 - limits[2]).ceil().clamp(0.0, rows as f64) as usize)
            .unwrap_or(0),
        clamp(limits[0], columns),
        limits[1]
            .is_finite()
            .then(|| limits[1].ceil().clamp(0.0, columns as f64) as usize)
            .unwrap_or(0),
    ]
}
pub(crate) fn heatmap_tile_bounds(
    rows: usize,
    columns: usize,
    visible: [usize; 4],
    max_cells: usize,
) -> [usize; 4] {
    let [r0, r1, c0, c1] = visible;
    let width = c1 - c0;
    let height = r1 - r0;
    let margin = ((max_cells.saturating_sub(width.saturating_mul(height))
        / width.saturating_add(height).saturating_add(2))
        / 2)
    .min(32);
    let snap_down = |v: usize| v / 32 * 32;
    let mut out = [
        snap_down(r0.saturating_sub(margin)),
        (r1 + margin).min(rows),
        snap_down(c0.saturating_sub(margin)),
        (c1 + margin).min(columns),
    ];
    out[1] = out[1].div_ceil(32).saturating_mul(32).min(rows);
    out[3] = out[3].div_ceil(32).saturating_mul(32).min(columns);
    if (out[1] - out[0]).saturating_mul(out[3] - out[2]) > max_cells {
        visible
    } else {
        out
    }
}

/// Read only the visible cells; plotting replacements do not alter stored values.
#[cfg(test)]
pub(crate) fn load_heatmap_tile(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &TableSlice,
    bounds: [usize; 4],
    replacement: f64,
) -> Result<HeatmapImage, String> {
    load_heatmap_tile_with_cancel(file, path, shape, slice, bounds, replacement, &|| false)
}

pub(crate) fn load_heatmap_tile_with_cancel(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &TableSlice,
    bounds: [usize; 4],
    replacement: f64,
    cancelled: &dyn Fn() -> bool,
) -> Result<HeatmapImage, String> {
    if cancelled() {
        return Err("2D loading cancelled".into());
    }
    let [r0, r1, c0, c1] = bounds;
    let (start, count) = slice.region(shape, r0, r1, c0, c1);
    let region = file
        .region(path, &start, &count)
        .map_err(|e| e.to_string())?;
    let mut values = Vec::with_capacity((r1 - r0) * (c1 - c0));
    let mut plotted = Vec::with_capacity(values.capacity());
    for row in r0..r1 {
        if cancelled() {
            return Err("2D loading cancelled".into());
        }
        for column in c0..c1 {
            let value = region.data[slice.region_offset(&count, row, column, &start)];
            if SpecialValue::from_value(value, region.fill_value).is_some() {
                plotted.push(replacement);
            } else {
                plotted.push(value);
            }
            values.push(value);
        }
    }
    let mut range = None;
    extend_finite_range(&mut range, &values, region.fill_value);
    Ok(HeatmapImage {
        bounds,
        grid: [r1 - r0, c1 - c0],
        values,
        plotted,
        fill_value: region.fill_value,
        range,
    })
}

pub(crate) fn overview_sample_index(bin: usize, source_len: usize, bins: usize) -> usize {
    (((bin as u128 * 2 + 1) * source_len as u128) / (bins as u128 * 2))
        .min((source_len - 1) as u128) as usize
}

#[cfg(test)]
pub(crate) fn load_heatmap_overview(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &TableSlice,
    grid: [usize; 2],
) -> Result<HeatmapImage, String> {
    load_heatmap_overview_with_cancel(file, path, shape, slice, grid, &|| false)
}

pub(crate) fn load_heatmap_overview_with_cancel(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &TableSlice,
    grid: [usize; 2],
    cancelled: &dyn Fn() -> bool,
) -> Result<HeatmapImage, String> {
    let rows = slice.rows(shape);
    let columns = slice.columns(shape);
    let sample_rows = (0..grid[0])
        .map(|bin| overview_sample_index(bin, rows, grid[0]))
        .collect::<Vec<_>>();
    let sample_columns = (0..grid[1])
        .map(|bin| overview_sample_index(bin, columns, grid[1]))
        .collect::<Vec<_>>();
    let mut values = vec![0.0; grid[0] * grid[1]];
    let mut plotted = vec![0.0; values.len()];
    let mut fill_value = None;
    let tile_columns = columns.min(256);
    let tile_rows = rows.min((65_536 / tile_columns).max(1));
    for row_start in (0..rows).step_by(tile_rows) {
        if cancelled() {
            return Err("2D loading cancelled".into());
        }
        let row_end = (row_start + tile_rows).min(rows);
        let first_row = sample_rows.partition_point(|&row| row < row_start);
        let last_row = sample_rows.partition_point(|&row| row < row_end);
        for column_start in (0..columns).step_by(tile_columns) {
            if cancelled() {
                return Err("2D loading cancelled".into());
            }
            let column_end = (column_start + tile_columns).min(columns);
            let first_column = sample_columns.partition_point(|&column| column < column_start);
            let last_column = sample_columns.partition_point(|&column| column < column_end);
            if first_row == last_row || first_column == last_column {
                continue;
            }
            let (start, count) = slice.region(shape, row_start, row_end, column_start, column_end);
            let region = file
                .region(path, &start, &count)
                .map_err(|err| err.to_string())?;
            fill_value = region.fill_value;
            for row_bin in first_row..last_row {
                for column_bin in first_column..last_column {
                    let value = region.data[slice.region_offset(
                        &count,
                        sample_rows[row_bin],
                        sample_columns[column_bin],
                        &start,
                    )];
                    let index = row_bin * grid[1] + column_bin;
                    values[index] = value;
                    if SpecialValue::from_value(value, region.fill_value).is_none() {
                        plotted[index] = value;
                    }
                }
            }
        }
    }
    let mut range = None;
    extend_finite_range(&mut range, &values, fill_value);
    Ok(HeatmapImage {
        bounds: [0, rows, 0, columns],
        grid,
        values,
        plotted,
        fill_value,
        range,
    })
}

#[cfg(test)]
pub(crate) fn scan_heatmap_range(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &TableSlice,
) -> Result<Option<(f64, f64)>, String> {
    scan_heatmap_range_with_cancel(file, path, shape, slice, &|| false)
}

pub(crate) fn scan_heatmap_range_with_cancel(
    file: &File,
    path: &str,
    shape: &[usize],
    slice: &TableSlice,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<(f64, f64)>, String> {
    let rows = slice.rows(shape);
    let columns = slice.columns(shape);
    if rows == 0 || columns == 0 {
        return Ok(None);
    }
    let tile_columns = columns.min(256);
    let tile_rows = rows.min((65_536 / tile_columns).max(1));
    let mut range = None;
    for row in (0..rows).step_by(tile_rows) {
        if cancelled() {
            return Err("2D loading cancelled".into());
        }
        for column in (0..columns).step_by(tile_columns) {
            if cancelled() {
                return Err("2D loading cancelled".into());
            }
            let (start, count) = slice.region(
                shape,
                row,
                (row + tile_rows).min(rows),
                column,
                (column + tile_columns).min(columns),
            );
            let region = file
                .region(path, &start, &count)
                .map_err(|err| err.to_string())?;
            extend_finite_range(&mut range, &region.data, region.fill_value);
        }
    }
    Ok(range)
}

//! Reading swaths as geolocated map points and optional pixel footprints.
use super::*;
use h5jive_data::ScanDirection;
use std::collections::HashMap;

/// Geographic center of the selected observations, including date-line crossings.
pub(crate) fn geospatial_center(
    points: &[MapPoint],
    scan_filter: h5jive_data::ScanFilter,
) -> Option<[f32; 2]> {
    let mut sum = [0.0_f64; 3];
    let mut count = 0;
    for point in points {
        if !scan_filter.allows(point.scan_direction)
            || !point.longitude.is_finite()
            || !point.latitude.is_finite()
            || !(-90.0..=90.0).contains(&point.latitude)
        {
            continue;
        }
        let lon = f64::from(point.longitude).to_radians();
        let lat = f64::from(point.latitude).to_radians();
        sum[0] += lat.cos() * lon.sin();
        sum[1] += lat.sin();
        sum[2] += lat.cos() * lon.cos();
        count += 1;
    }
    let length = sum[0].hypot(sum[1]).hypot(sum[2]);
    if count == 0 || length / f64::from(count) < 0.1 {
        return None;
    }
    Some([
        sum[0].atan2(sum[2]).to_degrees() as f32,
        sum[1].atan2(sum[0].hypot(sum[2])).to_degrees() as f32,
    ])
}

pub(crate) fn sampled_indexes(length: usize, wanted: usize) -> Vec<usize> {
    let count = length.min(wanted.max(1));
    if count == 1 {
        return vec![0];
    }
    (0..count)
        .map(|index| index * (length - 1) / (count - 1))
        .collect()
}

fn map_sample_indexes(
    rows: usize,
    columns: usize,
    budget: usize,
    full_swath_width: bool,
) -> (Vec<usize>, Vec<usize>) {
    let row_target = if full_swath_width {
        (budget / columns).max(1).min(rows)
    } else {
        rows.min((budget as f64 * rows as f64 / columns as f64).sqrt().ceil() as usize)
            .max(1)
    };
    let column_target = if full_swath_width {
        columns
    } else {
        columns.min((budget / row_target).max(1))
    };
    (
        sampled_indexes(rows, row_target),
        sampled_indexes(columns, column_target),
    )
}

fn sample_cell_edges(indexes: &[usize], length: usize) -> Vec<(usize, usize)> {
    indexes
        .iter()
        .enumerate()
        .map(|(index, &position)| {
            let first = if index == 0 {
                0
            } else {
                (indexes[index - 1] + position) / 2 + 1
            };
            let last = if index + 1 == indexes.len() {
                length - 1
            } else {
                (position + indexes[index + 1]) / 2
            };
            (first, last)
        })
        .collect()
}

#[cfg(test)]
mod cell_tests {
    use super::*;

    #[test]
    fn sampled_cells_cover_every_source_pixel_without_gaps() {
        for length in 1..100 {
            for wanted in 1..=length {
                let indexes = sampled_indexes(length, wanted);
                let cells = sample_cell_edges(&indexes, length);
                assert_eq!(cells.first().unwrap().0, 0);
                assert_eq!(cells.last().unwrap().1, length - 1);
                for pair in cells.windows(2) {
                    assert_eq!(pair[0].1 + 1, pair[1].0);
                }
            }
        }
    }

    #[test]
    fn swath_sampling_keeps_every_across_track_pixel() {
        let (rows, columns) = map_sample_indexes(1250, 32, 500, true);
        assert_eq!(rows.len(), 15);
        assert_eq!(columns, (0..32).collect::<Vec<_>>());
        let (_, grid_columns) = map_sample_indexes(1250, 32, 500, false);
        assert!(grid_columns.len() < 32);
    }

    #[test]
    fn cell_average_excludes_fill_and_nonfinite_values() {
        let values = Region {
            data: vec![1.0, 2.0, f64::NAN, 3.0, -999.0, 5.0],
            shape: vec![2, 3],
            fill_value: Some(-999.0),
        };
        assert_eq!(cell_average(&values, 3, (0, 1), (0, 2)), Some(2.75));
        assert_eq!(cell_average(&values, 3, (0, 0), (2, 2)), None);
    }

    #[test]
    fn padded_scanline_uses_only_valid_columns_for_tile_edges() {
        let latitudes = Region {
            data: vec![-999.0, 4.0, 4.0, 4.0, -999.0],
            shape: vec![1, 5],
            fill_value: Some(-999.0),
        };
        let longitudes = Region {
            data: vec![-999.0, 10.0, 11.0, 12.0, -999.0],
            shape: vec![1, 5],
            fill_value: Some(-999.0),
        };
        assert_eq!(
            valid_column_in_cell(&latitudes, &longitudes, 0, 5, (0, 2), 0),
            Some(1)
        );
        assert_eq!(
            valid_column_in_cell(&latitudes, &longitudes, 0, 5, (2, 4), 4),
            Some(3)
        );
        assert_eq!(
            valid_column_in_cell(&latitudes, &longitudes, 0, 5, (0, 0), 0),
            None
        );
    }

    #[test]
    fn padded_edge_row_uses_the_next_valid_scanline() {
        let latitudes = Region {
            data: vec![-999.0, -999.0, 3.0, 3.0],
            shape: vec![2, 2],
            fill_value: Some(-999.0),
        };
        let longitudes = Region {
            data: vec![-999.0, -999.0, 10.0, 11.0],
            shape: vec![2, 2],
            fill_value: Some(-999.0),
        };
        assert_eq!(
            valid_edge_pixel(&latitudes, &longitudes, 2, (0, 1), (0, 1), false, false),
            Some((1, 0))
        );
        assert_eq!(
            valid_edge_pixel(&latitudes, &longitudes, 2, (0, 1), (0, 1), false, true),
            Some((1, 1))
        );
        assert_eq!(
            valid_center_pixel(&latitudes, &longitudes, 2, (0, 1), (0, 1), 0, 0),
            Some((1, 0))
        );
    }

    #[test]
    fn outer_corner_uses_coordinate_direction_instead_of_corner_order() {
        let latitudes = Region {
            data: vec![0.5, 0.5, 1.5, 1.5],
            shape: vec![2, 2],
            fill_value: None,
        };
        let longitudes = Region {
            data: vec![0.5, 1.5, 0.5, 1.5],
            shape: vec![2, 2],
            fill_value: None,
        };
        // Corners are ordered northeast, southwest, northwest, southeast.
        let latitude_bounds = Region {
            data: vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0],
            shape: vec![2, 4],
            fill_value: None,
        };
        let longitude_bounds = Region {
            data: vec![1.0, 0.0, 0.0, 1.0, 2.0, 1.0, 1.0, 2.0],
            shape: vec![2, 4],
            fill_value: None,
        };
        assert_eq!(
            outer_pixel_corner(
                &latitude_bounds,
                &longitude_bounds,
                &latitudes,
                &longitudes,
                0,
                0,
                2,
                -1.0,
                -1.0,
            ),
            Some([0.0, 0.0])
        );
        assert_eq!(
            outer_pixel_corner(
                &latitude_bounds,
                &longitude_bounds,
                &latitudes,
                &longitudes,
                0,
                1,
                2,
                1.0,
                -1.0,
            ),
            Some([2.0, 0.0])
        );
    }

    #[test]
    fn mixed_scan_cells_are_not_labeled_as_one_direction() {
        let scan = ScanDirection {
            path: "scan_direction".into(),
            forward_values: vec![1],
            backward_values: vec![2],
            default_filter: h5jive_data::ScanFilter::Forward,
        };
        let values = Region {
            data: vec![1.0, 1.0, 1.0, 2.0],
            shape: vec![2, 2],
            fill_value: None,
        };
        assert_eq!(
            cell_scan_class(&scan, &values, 2, (0, 0), (0, 1)),
            ScanDirectionClass::Forward
        );
        assert_eq!(
            cell_scan_class(&scan, &values, 2, (0, 1), (0, 1)),
            ScanDirectionClass::Other
        );
    }
}

fn sample_bounds_line(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    row: usize,
    width: usize,
) -> Result<Region, String> {
    let (start, count) = if shape.len() == 4 {
        (vec![time, row, 0, 0], vec![1, 1, width, 4])
    } else {
        (vec![row, 0, 0], vec![1, width, 4])
    };
    file.region(path, &start, &count)
        .map_err(|error| error.to_string())
}

fn cell_average(
    values: &Region,
    width: usize,
    rows: (usize, usize),
    columns: (usize, usize),
) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0usize;
    for row in rows.0..=rows.1 {
        for column in columns.0..=columns.1 {
            if let Some(value) = finite_value(values.data[row * width + column], values.fill_value)
            {
                sum += value;
                count += 1;
            }
        }
    }
    (count > 0).then_some(sum / count as f64)
}

fn cell_scan_class(
    scan: &ScanDirection,
    values: &Region,
    width: usize,
    rows: (usize, usize),
    columns: (usize, usize),
) -> ScanDirectionClass {
    let mut class = None;
    for row in rows.0..=rows.1 {
        for column in columns.0..=columns.1 {
            let next = scan.classify(values.data[row * width + column], values.fill_value);
            if class.is_some_and(|previous| previous != next) {
                return ScanDirectionClass::Other;
            }
            class = Some(next);
        }
    }
    class.unwrap_or(ScanDirectionClass::Other)
}

fn longitude_delta(value: f64, reference: f64) -> f64 {
    (value - reference + 180.0).rem_euclid(360.0) - 180.0
}

fn valid_geo(latitudes: &Region, longitudes: &Region, index: usize) -> Option<(f64, f64)> {
    let latitude = finite_value(*latitudes.data.get(index)?, latitudes.fill_value)?;
    let longitude = finite_value(*longitudes.data.get(index)?, longitudes.fill_value)?;
    (-90.0..=90.0)
        .contains(&latitude)
        .then_some((latitude, longitude))
}

fn valid_column_in_cell(
    latitudes: &Region,
    longitudes: &Region,
    row: usize,
    width: usize,
    columns: (usize, usize),
    preferred: usize,
) -> Option<usize> {
    (columns.0..=columns.1)
        .filter(|&column| valid_geo(latitudes, longitudes, row * width + column).is_some())
        .min_by_key(|&column| column.abs_diff(preferred))
}

fn valid_edge_pixel(
    latitudes: &Region,
    longitudes: &Region,
    width: usize,
    rows: (usize, usize),
    columns: (usize, usize),
    from_bottom: bool,
    from_right: bool,
) -> Option<(usize, usize)> {
    let preferred = if from_right { columns.1 } else { columns.0 };
    let mut search = rows.0..=rows.1;
    if from_bottom {
        search.rev().find_map(|row| {
            valid_column_in_cell(latitudes, longitudes, row, width, columns, preferred)
                .map(|column| (row, column))
        })
    } else {
        search.find_map(|row| {
            valid_column_in_cell(latitudes, longitudes, row, width, columns, preferred)
                .map(|column| (row, column))
        })
    }
}

fn valid_center_pixel(
    latitudes: &Region,
    longitudes: &Region,
    width: usize,
    rows: (usize, usize),
    columns: (usize, usize),
    preferred_row: usize,
    preferred_column: usize,
) -> Option<(usize, usize)> {
    (rows.0..=rows.1)
        .filter_map(|row| {
            valid_column_in_cell(latitudes, longitudes, row, width, columns, preferred_column)
                .map(|column| (row, column))
        })
        .min_by_key(|&(row, column)| {
            (
                row.abs_diff(preferred_row),
                column.abs_diff(preferred_column),
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn cached_outer_corner(
    file: &File,
    latitude_path: &str,
    longitude_path: &str,
    latitude_shape: &[usize],
    longitude_shape: &[usize],
    time: usize,
    width: usize,
    cache: &mut HashMap<usize, (Region, Region)>,
    latitudes: &Region,
    longitudes: &Region,
    row: usize,
    column: usize,
    x_sign: f64,
    y_sign: f64,
) -> Result<Option<[f32; 2]>, String> {
    if let std::collections::hash_map::Entry::Vacant(entry) = cache.entry(row) {
        entry.insert((
            sample_bounds_line(file, latitude_path, latitude_shape, time, row, width)?,
            sample_bounds_line(file, longitude_path, longitude_shape, time, row, width)?,
        ));
    }
    let (latitude_bounds, longitude_bounds) = &cache[&row];
    Ok(outer_pixel_corner(
        latitude_bounds,
        longitude_bounds,
        latitudes,
        longitudes,
        row,
        column,
        width,
        x_sign,
        y_sign,
    ))
}

fn outer_pixel_corner(
    latitude_bounds: &Region,
    longitude_bounds: &Region,
    latitudes: &Region,
    longitudes: &Region,
    row: usize,
    column: usize,
    width: usize,
    x_sign: f64,
    y_sign: f64,
) -> Option<[f32; 2]> {
    let index = row * width + column;
    let (latitude, longitude) = valid_geo(latitudes, longitudes, index)?;
    let height = latitudes.data.len() / width;
    let neighbor_x = (1..width).find_map(|distance| {
        [
            column.checked_add(distance).filter(|&x| x < width),
            column.checked_sub(distance),
        ]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            valid_geo(latitudes, longitudes, row * width + candidate)
                .map(|coords| (candidate, coords))
        })
    })?;
    let neighbor_y = (1..height).find_map(|distance| {
        [
            row.checked_add(distance).filter(|&y| y < height),
            row.checked_sub(distance),
        ]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            valid_geo(latitudes, longitudes, candidate * width + column)
                .map(|coords| (candidate, coords))
        })
    })?;
    let (x_latitude, x_longitude) = neighbor_x.1;
    let (y_latitude, y_longitude) = neighbor_y.1;
    let x_step = neighbor_x.0 as f64 - column as f64;
    let y_step = neighbor_y.0 as f64 - row as f64;
    let lon_scale = latitude.to_radians().cos().abs().max(0.05);
    let expected_x = 0.5
        * (x_sign * longitude_delta(x_longitude, longitude) / x_step
            + y_sign * longitude_delta(y_longitude, longitude) / y_step)
        * lon_scale;
    let expected_y = 0.5
        * (x_sign * (x_latitude - latitude) / x_step + y_sign * (y_latitude - latitude) / y_step);
    (0..4)
        .filter_map(|corner| {
            let bound_latitude = *latitude_bounds.data.get(column * 4 + corner)?;
            let bound_longitude = *longitude_bounds.data.get(column * 4 + corner)?;
            if !(-90.0..=90.0).contains(&bound_latitude) || !bound_longitude.is_finite() {
                return None;
            }
            let dx = longitude_delta(bound_longitude, longitude) * lon_scale - expected_x;
            let dy = bound_latitude - latitude - expected_y;
            Some((
                dx * dx + dy * dy,
                [bound_longitude as f32, bound_latitude as f32],
            ))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, [longitude, latitude])| {
            [((longitude + 180.0).rem_euclid(360.0) - 180.0), latitude]
        })
}

fn sample_line(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    row: usize,
    width: usize,
    spectral: usize,
) -> Result<Region, String> {
    let (start, count) = if shape.len() == 4 {
        (vec![time, row, 0, spectral], vec![1, 1, width, 1])
    } else if shape.len() == 3 {
        (vec![time, row, 0], vec![1, 1, width])
    } else {
        (vec![row, 0], vec![1, width])
    };
    file.region(path, &start, &count)
        .map_err(|error| error.to_string())
}

fn sample_coordinate_line(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    row: usize,
    width: usize,
    latitude: bool,
) -> Result<Region, String> {
    if shape.len() != 1 {
        return sample_line(file, path, shape, time, row, width, 0);
    }
    let (start, count) = if latitude {
        (vec![row], vec![1])
    } else {
        (vec![0], vec![width])
    };
    let region = file
        .region(path, &start, &count)
        .map_err(|error| error.to_string())?;
    Ok(Region {
        data: if latitude {
            vec![region.data[0]; width]
        } else {
            region.data
        },
        shape: vec![width],
        fill_value: region.fill_value,
    })
}

fn sample_coordinate_rows(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    rows: usize,
    width: usize,
    latitude: bool,
) -> Result<Region, String> {
    if shape.len() != 1 {
        return sample_all_rows(file, path, shape, time, rows, width, 0);
    }
    let region = file
        .region(path, &[0], &[shape[0]])
        .map_err(|error| error.to_string())?;
    let mut data = Vec::with_capacity(rows * width);
    if latitude {
        for value in region.data {
            data.extend(std::iter::repeat_n(value, width));
        }
    } else {
        for _ in 0..rows {
            data.extend_from_slice(&region.data);
        }
    }
    Ok(Region {
        data,
        shape: vec![rows, width],
        fill_value: region.fill_value,
    })
}

fn sample_coordinate_plane(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    rows: usize,
    width: usize,
    latitude: bool,
) -> Result<Region, String> {
    if shape.len() == 1 {
        sample_coordinate_rows(file, path, shape, time, rows, width, latitude)
    } else {
        sample_spatial_plane(file, path, shape, time, rows, width, 0)
    }
}

fn sample_all_rows(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    rows: usize,
    width: usize,
    spectral: usize,
) -> Result<Region, String> {
    let (start, count) = if shape.len() == 4 {
        (vec![time, 0, 0, spectral], vec![1, rows, width, 1])
    } else if shape.len() == 3 {
        (vec![time, 0, 0], vec![1, rows, width])
    } else {
        (vec![0, 0], vec![rows, width])
    };
    file.region(path, &start, &count)
        .map_err(|error| error.to_string())
}

fn sample_spatial_plane(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    rows: usize,
    width: usize,
    spectral: usize,
) -> Result<Region, String> {
    sample_spatial_plane_with_cancel(file, path, shape, time, rows, width, spectral, &|| false)
}

fn sample_spatial_plane_with_cancel(
    file: &File,
    path: &str,
    shape: &[usize],
    time: usize,
    rows: usize,
    width: usize,
    spectral: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Region, String> {
    let rows_per_read = (1_000_000 / width).clamp(1, 512);
    let mut data = Vec::with_capacity(rows * width);
    let mut fill_value = None;
    for first in (0..rows).step_by(rows_per_read) {
        if cancelled() {
            return Err("Map loading cancelled".into());
        }
        let count_rows = (rows - first).min(rows_per_read);
        let (start, count) = if shape.len() == 4 {
            (
                vec![time, first, 0, spectral],
                vec![1, count_rows, width, 1],
            )
        } else if shape.len() == 3 {
            (vec![time, first, 0], vec![1, count_rows, width])
        } else {
            (vec![first, 0], vec![count_rows, width])
        };
        let region = file
            .region(path, &start, &count)
            .map_err(|error| error.to_string())?;
        fill_value = region.fill_value;
        data.extend(region.data);
    }
    Ok(Region {
        data,
        shape: vec![rows, width],
        fill_value,
    })
}

fn batch_row(batch: &Region, row: usize, width: usize) -> Region {
    let start = row * width;
    Region {
        data: batch.data[start..start + width].to_vec(),
        shape: vec![width],
        fill_value: batch.fill_value,
    }
}

/// Read the selected spectral plane with a point budget shared across times.
pub(crate) fn load_map_data(
    file: &File,
    path: &str,
    shape: &[usize],
    geolocation: &Geolocation,
    time: Option<usize>,
    spectral: usize,
    max_points: usize,
) -> Result<MapData, String> {
    load_map_data_with_batch_limit(
        file,
        path,
        shape,
        geolocation,
        time,
        spectral,
        max_points,
        250_000,
    )
}

pub(crate) fn load_map_data_with_batch_limit(
    file: &File,
    path: &str,
    shape: &[usize],
    geolocation: &Geolocation,
    time: Option<usize>,
    spectral: usize,
    max_points: usize,
    batch_limit: usize,
) -> Result<MapData, String> {
    load_map_data_inner(
        file,
        path,
        shape,
        geolocation,
        time,
        spectral,
        max_points,
        batch_limit,
        None,
        false,
    )
}

pub(crate) fn load_map_preview(
    file: &File,
    path: &str,
    shape: &[usize],
    geolocation: &Geolocation,
    time: Option<usize>,
    spectral: usize,
    max_points: usize,
) -> Result<MapData, String> {
    load_map_data_inner(
        file,
        path,
        shape,
        geolocation,
        time,
        spectral,
        max_points,
        250_000,
        None,
        true,
    )
}

pub(crate) fn load_map_data_with_radiance(
    file: &File,
    path: &str,
    shape: &[usize],
    geolocation: &Geolocation,
    time: Option<usize>,
    spectral: usize,
    max_points: usize,
    radiance: &Region,
) -> Result<MapData, String> {
    load_map_data_inner(
        file,
        path,
        shape,
        geolocation,
        time,
        spectral,
        max_points,
        250_000,
        Some(radiance),
        false,
    )
}

#[cfg(test)]
pub(crate) fn read_map_radiance_plane(
    file: &File,
    path: &str,
    shape: &[usize],
    time: Option<usize>,
    spectral: usize,
) -> Result<Region, String> {
    read_map_radiance_plane_with_cancel(file, path, shape, time, spectral, &|| false)
}

pub(crate) fn read_map_radiance_plane_with_cancel(
    file: &File,
    path: &str,
    shape: &[usize],
    time: Option<usize>,
    spectral: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Region, String> {
    let (rows, width) = if shape.len() >= 3 {
        (shape[1], shape[2])
    } else {
        (shape[0], shape[1])
    };
    sample_spatial_plane_with_cancel(
        file,
        path,
        shape,
        time.unwrap_or(0),
        rows,
        width,
        spectral,
        cancelled,
    )
}

fn load_map_data_inner(
    file: &File,
    path: &str,
    shape: &[usize],
    geolocation: &Geolocation,
    time: Option<usize>,
    spectral: usize,
    max_points: usize,
    batch_limit: usize,
    radiance: Option<&Region>,
    preview: bool,
) -> Result<MapData, String> {
    if shape.iter().any(|&size| size == 0) {
        return Err("Empty dataset".into());
    }
    let geo_lat = file
        .dataset_info(&geolocation.latitude)
        .map_err(|error| error.to_string())?
        .shape;
    let geo_lon = file
        .dataset_info(&geolocation.longitude)
        .map_err(|error| error.to_string())?
        .shape;
    let scan_shape = geolocation
        .scan_direction
        .as_ref()
        .map(|scan| file.dataset_info(&scan.path).map(|info| info.shape))
        .transpose()
        .map_err(|error| error.to_string())?;
    let bounds_lat = geolocation
        .latitude_bounds
        .as_ref()
        .map(|path| file.dataset_info(path).map(|info| info.shape))
        .transpose()
        .map_err(|e| e.to_string())?;
    let bounds_lon = geolocation
        .longitude_bounds
        .as_ref()
        .map(|path| file.dataset_info(path).map(|info| info.shape))
        .transpose()
        .map_err(|e| e.to_string())?;
    let (nt, ny, nx) = if shape.len() >= 3 {
        (shape[0], shape[1], shape[2])
    } else {
        (1, shape[0], shape[1])
    };
    let times: Vec<usize> = time.map_or_else(|| (0..nt).collect(), |index| vec![index]);
    let budget_per_time = (max_points / times.len()).max(1);
    let full_swath_width = geo_lat.len() > 1 && geo_lon.len() > 1;
    let (rows, columns) = map_sample_indexes(ny, nx, budget_per_time, full_swath_width);
    let aggregate_footprints = (!preview || ny.saturating_mul(nx) <= MAX_PLANE_CELLS)
        && (rows.len() < ny || columns.len() < nx)
        && nx > 1
        && ny > 1
        && bounds_lat.is_some()
        && bounds_lon.is_some();
    let aggregate_values = aggregate_footprints && !preview;
    let row_cells = sample_cell_edges(&rows, ny);
    let column_cells = sample_cell_edges(&columns, nx);
    // Dense, modest swaths are much faster as a few hyperslabs than thousands of tiny reads.
    let batch_rows = rows.len() == ny && ny.saturating_mul(nx) <= batch_limit;
    let mut points = Vec::with_capacity(times.len() * rows.len() * columns.len());
    let mut range: Option<(f64, f64)> = None;
    for t in times {
        let spectral = spectral.min(shape[shape.len() - 1] - 1);
        let all_values_owned = (aggregate_values && radiance.is_none())
            .then(|| sample_spatial_plane(file, path, shape, t, ny, nx, spectral))
            .transpose()?;
        let all_values = radiance.or(all_values_owned.as_ref());
        let all_latitudes = aggregate_footprints
            .then(|| {
                sample_coordinate_plane(
                    file,
                    &geolocation.latitude,
                    &geo_lat,
                    if geo_lat.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    true,
                )
            })
            .transpose()?;
        let all_longitudes = aggregate_footprints
            .then(|| {
                sample_coordinate_plane(
                    file,
                    &geolocation.longitude,
                    &geo_lon,
                    if geo_lon.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    false,
                )
            })
            .transpose()?;
        let all_scan_values = if aggregate_footprints {
            geolocation
                .scan_direction
                .as_ref()
                .zip(scan_shape.as_ref())
                .map(|(scan, shape)| {
                    sample_spatial_plane(
                        file,
                        &scan.path,
                        shape,
                        if shape.len() >= 3 { t } else { 0 },
                        ny,
                        nx,
                        0,
                    )
                })
                .transpose()?
        } else {
            None
        };
        let values_batch = (batch_rows && all_values.is_none())
            .then(|| sample_all_rows(file, path, shape, t, ny, nx, spectral))
            .transpose()?;
        let latitudes_batch = batch_rows
            .then(|| {
                sample_coordinate_rows(
                    file,
                    &geolocation.latitude,
                    &geo_lat,
                    if geo_lat.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    true,
                )
            })
            .transpose()?;
        let longitudes_batch = batch_rows
            .then(|| {
                sample_coordinate_rows(
                    file,
                    &geolocation.longitude,
                    &geo_lon,
                    if geo_lon.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    false,
                )
            })
            .transpose()?;
        let scan_batch = if batch_rows {
            geolocation
                .scan_direction
                .as_ref()
                .zip(scan_shape.as_ref())
                .map(|(scan, scan_shape)| {
                    sample_all_rows(
                        file,
                        &scan.path,
                        scan_shape,
                        if scan_shape.len() >= 3 { t } else { 0 },
                        ny,
                        nx,
                        0,
                    )
                })
                .transpose()?
        } else {
            None
        };
        let bounds_batch = if batch_rows {
            if let (Some(lat_path), Some(lon_path), Some(lat_shape), Some(lon_shape)) = (
                &geolocation.latitude_bounds,
                &geolocation.longitude_bounds,
                &bounds_lat,
                &bounds_lon,
            ) {
                let read = |path: &str, shape: &[usize]| -> Result<Region, String> {
                    let (start, count) = if shape.len() == 4 && shape[0] == nt {
                        (vec![t, 0, 0, 0], vec![1, ny, nx, 4])
                    } else {
                        (vec![0, 0, 0], vec![ny, nx, 4])
                    };
                    file.region(path, &start, &count)
                        .map_err(|error| error.to_string())
                };
                Some((read(lat_path, lat_shape)?, read(lon_path, lon_shape)?))
            } else {
                None
            }
        } else {
            None
        };
        let mut bounds_cache = HashMap::new();
        let mut value_row_cache = HashMap::new();
        for (row_index, &y) in rows.iter().enumerate() {
            // One contiguous row read per variable keeps both HDF5 and NetCDF access bounded.
            let values = if let Some(batch) = all_values.or(values_batch.as_ref()) {
                batch_row(batch, y, nx)
            } else {
                sample_line(file, path, shape, t, y, nx, spectral)?
            };
            let latitudes = if let Some(batch) = all_latitudes.as_ref().or(latitudes_batch.as_ref())
            {
                batch_row(batch, y, nx)
            } else {
                sample_coordinate_line(
                    file,
                    &geolocation.latitude,
                    &geo_lat,
                    if geo_lat.len() == 3 { t } else { 0 },
                    y,
                    nx,
                    true,
                )?
            };
            let longitudes =
                if let Some(batch) = all_longitudes.as_ref().or(longitudes_batch.as_ref()) {
                    batch_row(batch, y, nx)
                } else {
                    sample_coordinate_line(
                        file,
                        &geolocation.longitude,
                        &geo_lon,
                        if geo_lon.len() == 3 { t } else { 0 },
                        y,
                        nx,
                        false,
                    )?
                };
            let scan_values = if let Some(batch) = all_scan_values.as_ref().or(scan_batch.as_ref())
            {
                Some(batch_row(batch, y, nx))
            } else {
                geolocation
                    .scan_direction
                    .as_ref()
                    .zip(scan_shape.as_ref())
                    .map(|(scan, scan_shape)| {
                        sample_line(
                            file,
                            &scan.path,
                            scan_shape,
                            if scan_shape.len() >= 3 { t } else { 0 },
                            y,
                            nx,
                            0,
                        )
                    })
                    .transpose()?
            };
            let corners = if aggregate_footprints {
                None
            } else if let Some((lat, lon)) = &bounds_batch {
                Some((batch_row(lat, y, nx * 4), batch_row(lon, y, nx * 4)))
            } else if let (Some(lat_path), Some(lon_path), Some(lat_shape), Some(lon_shape)) = (
                &geolocation.latitude_bounds,
                &geolocation.longitude_bounds,
                &bounds_lat,
                &bounds_lon,
            ) {
                let lat = sample_bounds_line(file, lat_path, lat_shape, t, y, nx)?;
                let lon = sample_bounds_line(file, lon_path, lon_shape, t, y, nx)?;
                Some((lat, lon))
            } else {
                None
            };
            for (column_index, &sampled_x) in columns.iter().enumerate() {
                let (center_row, x) = if aggregate_footprints {
                    let latitudes = all_latitudes.as_ref().expect("aggregated latitude plane");
                    let longitudes = all_longitudes.as_ref().expect("aggregated longitude plane");
                    let Some(pixel) = valid_center_pixel(
                        latitudes,
                        longitudes,
                        nx,
                        row_cells[row_index],
                        column_cells[column_index],
                        y,
                        sampled_x,
                    ) else {
                        continue;
                    };
                    pixel
                } else {
                    (y, sampled_x)
                };
                let latitude = if center_row == y {
                    latitudes.data[x]
                } else {
                    all_latitudes
                        .as_ref()
                        .expect("aggregated latitude plane")
                        .data[center_row * nx + x]
                };
                let longitude = if center_row == y {
                    longitudes.data[x]
                } else {
                    all_longitudes
                        .as_ref()
                        .expect("aggregated longitude plane")
                        .data[center_row * nx + x]
                };
                if !latitude.is_finite()
                    || !longitude.is_finite()
                    || !(-90.0..=90.0).contains(&latitude)
                {
                    continue;
                }
                let sampled_value = if center_row == y {
                    values.data[x]
                } else if let Some(all) = all_values {
                    all.data[center_row * nx + x]
                } else {
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        value_row_cache.entry(center_row)
                    {
                        entry.insert(sample_line(file, path, shape, t, center_row, nx, spectral)?);
                    }
                    value_row_cache[&center_row].data[x]
                };
                let value = aggregate_values
                    .then(|| {
                        all_values.and_then(|all| {
                            cell_average(all, nx, row_cells[row_index], column_cells[column_index])
                        })
                    })
                    .flatten()
                    .unwrap_or(sampled_value);
                let special = SpecialValue::from_value(value, values.fill_value);
                if let Some(finite) = finite_value(value, values.fill_value) {
                    range = Some(range.map_or((finite, finite), |(lo, hi)| {
                        (lo.min(finite), hi.max(finite))
                    }));
                }
                let footprint = if aggregate_footprints {
                    let latitudes = all_latitudes.as_ref().expect("aggregated latitude plane");
                    let longitudes = all_longitudes.as_ref().expect("aggregated longitude plane");
                    let (Some(lat_path), Some(lon_path), Some(lat_shape), Some(lon_shape)) = (
                        &geolocation.latitude_bounds,
                        &geolocation.longitude_bounds,
                        &bounds_lat,
                        &bounds_lon,
                    ) else {
                        unreachable!("aggregated footprints require bounds");
                    };
                    let rows = row_cells[row_index];
                    let columns = column_cells[column_index];
                    let corners = [
                        valid_edge_pixel(latitudes, longitudes, nx, rows, columns, false, false),
                        valid_edge_pixel(latitudes, longitudes, nx, rows, columns, false, true),
                        valid_edge_pixel(latitudes, longitudes, nx, rows, columns, true, true),
                        valid_edge_pixel(latitudes, longitudes, nx, rows, columns, true, false),
                    ];
                    if let [Some(top_left), Some(top_right), Some(bottom_right), Some(bottom_left)] =
                        corners
                    {
                        let mut polygon = [[0.0; 2]; 4];
                        let pixels = [top_left, top_right, bottom_right, bottom_left];
                        let signs = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
                        let mut complete = true;
                        for (index, ((row, column), (x_sign, y_sign))) in
                            pixels.into_iter().zip(signs).enumerate()
                        {
                            if let Some(corner) = cached_outer_corner(
                                file,
                                lat_path,
                                lon_path,
                                lat_shape,
                                lon_shape,
                                t,
                                nx,
                                &mut bounds_cache,
                                latitudes,
                                longitudes,
                                row,
                                column,
                                x_sign,
                                y_sign,
                            )? {
                                polygon[index] = corner;
                            } else {
                                complete = false;
                                break;
                            }
                        }
                        complete.then_some(polygon)
                    } else {
                        None
                    }
                } else {
                    corners.as_ref().and_then(|(lat, lon)| {
                        let mut polygon = [[0.0; 2]; 4];
                        for corner in 0..4 {
                            let index = x * 4 + corner;
                            let latitude = *lat.data.get(index)? as f32;
                            let longitude = *lon.data.get(index)? as f32;
                            if !(-90.0..=90.0).contains(&latitude) || !longitude.is_finite() {
                                return None;
                            }
                            polygon[corner] =
                                [((longitude + 180.0).rem_euclid(360.0) - 180.0), latitude];
                        }
                        Some(polygon)
                    })
                };
                points.push(MapPoint {
                    latitude: latitude as f32,
                    longitude: ((longitude + 180.0).rem_euclid(360.0) - 180.0) as f32,
                    value,
                    special,
                    scan_direction: geolocation.scan_direction.as_ref().and_then(|scan| {
                        if let Some(all) = &all_scan_values {
                            Some(cell_scan_class(
                                scan,
                                all,
                                nx,
                                row_cells[row_index],
                                column_cells[column_index],
                            ))
                        } else {
                            scan_values
                                .as_ref()
                                .map(|values| scan.classify(values.data[x], values.fill_value))
                        }
                    }),
                    footprint,
                });
            }
        }
    }
    Ok(MapData {
        points,
        range,
        aggregated: aggregate_footprints,
        sampled: rows.len() < ny || columns.len() < nx,
        preview,
    })
}

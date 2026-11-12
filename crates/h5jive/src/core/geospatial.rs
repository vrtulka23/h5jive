//! Reading swaths as geolocated map points and optional pixel footprints.
use super::*;
use h5jive_data::ScanDirection;

pub(crate) fn sampled_indexes(length: usize, wanted: usize) -> Vec<usize> {
    let count = length.min(wanted.max(1));
    if count == 1 {
        return vec![0];
    }
    (0..count)
        .map(|index| index * (length - 1) / (count - 1))
        .collect()
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
    let latitude = *latitudes.data.get(index)?;
    let longitude = *longitudes.data.get(index)?;
    if !(-90.0..=90.0).contains(&latitude) || !longitude.is_finite() {
        return None;
    }
    let neighbor_x = if column + 1 < width {
        column + 1
    } else {
        column.saturating_sub(1)
    };
    let neighbor_y = if row + 1 < latitudes.data.len() / width {
        row + 1
    } else {
        row.saturating_sub(1)
    };
    let x_step = if neighbor_x > column { 1.0 } else { -1.0 };
    let y_step = if neighbor_y > row { 1.0 } else { -1.0 };
    let x_index = row * width + neighbor_x;
    let y_index = neighbor_y * width + column;
    let x_latitude = *latitudes.data.get(x_index)?;
    let x_longitude = *longitudes.data.get(x_index)?;
    let y_latitude = *latitudes.data.get(y_index)?;
    let y_longitude = *longitudes.data.get(y_index)?;
    if !(-90.0..=90.0).contains(&x_latitude)
        || !(-90.0..=90.0).contains(&y_latitude)
        || !x_longitude.is_finite()
        || !y_longitude.is_finite()
    {
        return None;
    }
    let lon_scale = latitude.to_radians().cos().abs().max(0.05);
    let expected_x = 0.5
        * (x_sign * longitude_delta(x_longitude, longitude) * x_step
            + y_sign * longitude_delta(y_longitude, longitude) * y_step)
        * lon_scale;
    let expected_y = 0.5
        * (x_sign * (x_latitude - latitude) * x_step + y_sign * (y_latitude - latitude) * y_step);
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
    let rows_per_read = (1_000_000 / width).max(1);
    let mut data = Vec::with_capacity(rows * width);
    let mut fill_value = None;
    for first in (0..rows).step_by(rows_per_read) {
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
    let row_target = ny
        .min(
            (budget_per_time as f64 * ny as f64 / nx as f64)
                .sqrt()
                .ceil() as usize,
        )
        .max(1);
    let col_target = nx.min((budget_per_time / row_target).max(1));
    let rows = sampled_indexes(ny, row_target);
    let columns = sampled_indexes(nx, col_target);
    let aggregate_footprints = (rows.len() < ny || columns.len() < nx)
        && nx > 1
        && ny > 1
        && bounds_lat.is_some()
        && bounds_lon.is_some();
    let row_cells = sample_cell_edges(&rows, ny);
    let column_cells = sample_cell_edges(&columns, nx);
    // Dense, modest swaths are much faster as a few hyperslabs than thousands of tiny reads.
    let batch_rows = rows.len() == ny && ny.saturating_mul(nx) <= batch_limit;
    let mut points = Vec::with_capacity(times.len() * rows.len() * columns.len());
    let mut range: Option<(f64, f64)> = None;
    for t in times {
        let spectral = spectral.min(shape[shape.len() - 1] - 1);
        let all_values = aggregate_footprints
            .then(|| sample_spatial_plane(file, path, shape, t, ny, nx, spectral))
            .transpose()?;
        let all_latitudes = aggregate_footprints
            .then(|| {
                sample_spatial_plane(
                    file,
                    &geolocation.latitude,
                    &geo_lat,
                    if geo_lat.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    0,
                )
            })
            .transpose()?;
        let all_longitudes = aggregate_footprints
            .then(|| {
                sample_spatial_plane(
                    file,
                    &geolocation.longitude,
                    &geo_lon,
                    if geo_lon.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    0,
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
        let values_batch = batch_rows
            .then(|| sample_all_rows(file, path, shape, t, ny, nx, spectral))
            .transpose()?;
        let latitudes_batch = batch_rows
            .then(|| {
                sample_all_rows(
                    file,
                    &geolocation.latitude,
                    &geo_lat,
                    if geo_lat.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    0,
                )
            })
            .transpose()?;
        let longitudes_batch = batch_rows
            .then(|| {
                sample_all_rows(
                    file,
                    &geolocation.longitude,
                    &geo_lon,
                    if geo_lon.len() == 3 { t } else { 0 },
                    ny,
                    nx,
                    0,
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
        for (row_index, &y) in rows.iter().enumerate() {
            // One contiguous row read per variable keeps both HDF5 and NetCDF access bounded.
            let values = if let Some(batch) = all_values.as_ref().or(values_batch.as_ref()) {
                batch_row(batch, y, nx)
            } else {
                sample_line(file, path, shape, t, y, nx, spectral)?
            };
            let latitudes = if let Some(batch) = all_latitudes.as_ref().or(latitudes_batch.as_ref())
            {
                batch_row(batch, y, nx)
            } else {
                sample_line(
                    file,
                    &geolocation.latitude,
                    &geo_lat,
                    if geo_lat.len() == 3 { t } else { 0 },
                    y,
                    nx,
                    0,
                )?
            };
            let longitudes =
                if let Some(batch) = all_longitudes.as_ref().or(longitudes_batch.as_ref()) {
                    batch_row(batch, y, nx)
                } else {
                    sample_line(
                        file,
                        &geolocation.longitude,
                        &geo_lon,
                        if geo_lon.len() == 3 { t } else { 0 },
                        y,
                        nx,
                        0,
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
            let tile_corners = if aggregate_footprints {
                if let (Some(lat_path), Some(lon_path), Some(lat_shape), Some(lon_shape)) = (
                    &geolocation.latitude_bounds,
                    &geolocation.longitude_bounds,
                    &bounds_lat,
                    &bounds_lon,
                ) {
                    let (first, last) = row_cells[row_index];
                    Some((
                        sample_bounds_line(file, lat_path, lat_shape, t, first, nx)?,
                        sample_bounds_line(file, lon_path, lon_shape, t, first, nx)?,
                        sample_bounds_line(file, lat_path, lat_shape, t, last, nx)?,
                        sample_bounds_line(file, lon_path, lon_shape, t, last, nx)?,
                    ))
                } else {
                    None
                }
            } else {
                None
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
            for (column_index, &x) in columns.iter().enumerate() {
                let latitude = latitudes.data[x];
                let longitude = longitudes.data[x];
                if !latitude.is_finite()
                    || !longitude.is_finite()
                    || !(-90.0..=90.0).contains(&latitude)
                {
                    continue;
                }
                let value = all_values
                    .as_ref()
                    .and_then(|all| {
                        cell_average(all, nx, row_cells[row_index], column_cells[column_index])
                    })
                    .unwrap_or(values.data[x]);
                let special = SpecialValue::from_value(value, values.fill_value);
                if let Some(finite) = finite_value(value, values.fill_value) {
                    range = Some(range.map_or((finite, finite), |(lo, hi)| {
                        (lo.min(finite), hi.max(finite))
                    }));
                }
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
                    footprint: tile_corners
                        .as_ref()
                        .and_then(|(top_lat, top_lon, bottom_lat, bottom_lon)| {
                            let latitudes = all_latitudes.as_ref()?;
                            let longitudes = all_longitudes.as_ref()?;
                            let (first_row, last_row) = row_cells[row_index];
                            let (first_column, last_column) = column_cells[column_index];
                            Some([
                                outer_pixel_corner(
                                    top_lat,
                                    top_lon,
                                    latitudes,
                                    longitudes,
                                    first_row,
                                    first_column,
                                    nx,
                                    -1.0,
                                    -1.0,
                                )?,
                                outer_pixel_corner(
                                    top_lat,
                                    top_lon,
                                    latitudes,
                                    longitudes,
                                    first_row,
                                    last_column,
                                    nx,
                                    1.0,
                                    -1.0,
                                )?,
                                outer_pixel_corner(
                                    bottom_lat,
                                    bottom_lon,
                                    latitudes,
                                    longitudes,
                                    last_row,
                                    last_column,
                                    nx,
                                    1.0,
                                    1.0,
                                )?,
                                outer_pixel_corner(
                                    bottom_lat,
                                    bottom_lon,
                                    latitudes,
                                    longitudes,
                                    last_row,
                                    first_column,
                                    nx,
                                    -1.0,
                                    1.0,
                                )?,
                            ])
                        })
                        .or_else(|| {
                            corners.as_ref().and_then(|(lat, lon)| {
                                let mut polygon = [[0.0; 2]; 4];
                                for corner in 0..4 {
                                    let index = x * 4 + corner;
                                    let latitude = *lat.data.get(index)? as f32;
                                    let longitude = *lon.data.get(index)? as f32;
                                    if !(-90.0..=90.0).contains(&latitude) || !longitude.is_finite()
                                    {
                                        return None;
                                    }
                                    polygon[corner] =
                                        [((longitude + 180.0).rem_euclid(360.0) - 180.0), latitude];
                                }
                                Some(polygon)
                            })
                        }),
                });
            }
        }
    }
    Ok(MapData {
        points,
        range,
        aggregated: aggregate_footprints,
    })
}

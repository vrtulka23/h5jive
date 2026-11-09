//! Reading swaths as geolocated map points and optional pixel footprints.
use super::*;

pub(crate) fn sampled_indexes(length: usize, wanted: usize) -> Vec<usize> {
    let count = length.min(wanted.max(1));
    if count == 1 {
        return vec![0];
    }
    (0..count)
        .map(|index| index * (length - 1) / (count - 1))
        .collect()
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
    // Dense, modest swaths are much faster as a few hyperslabs than thousands of tiny reads.
    let batch_rows = rows.len() == ny && ny.saturating_mul(nx) <= batch_limit;
    let mut points = Vec::with_capacity(times.len() * rows.len() * columns.len());
    let mut range: Option<(f64, f64)> = None;
    for t in times {
        let spectral = spectral.min(shape[shape.len() - 1] - 1);
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
        for &y in &rows {
            // One contiguous row read per variable keeps both HDF5 and NetCDF access bounded.
            let values = if let Some(batch) = &values_batch {
                batch_row(batch, y, nx)
            } else {
                sample_line(file, path, shape, t, y, nx, spectral)?
            };
            let latitudes = if let Some(batch) = &latitudes_batch {
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
            let longitudes = if let Some(batch) = &longitudes_batch {
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
            let scan_values = if let Some(batch) = &scan_batch {
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
            let corners = if let Some((lat, lon)) = &bounds_batch {
                Some((batch_row(lat, y, nx * 4), batch_row(lon, y, nx * 4)))
            } else if let (Some(lat_path), Some(lon_path), Some(lat_shape), Some(lon_shape)) = (
                &geolocation.latitude_bounds,
                &geolocation.longitude_bounds,
                &bounds_lat,
                &bounds_lon,
            ) {
                let read = |path: &str, shape: &[usize]| -> Result<Region, String> {
                    let (start, count) = if shape.len() == 4 && shape[0] == nt {
                        (vec![t, y, 0, 0], vec![1, 1, nx, 4])
                    } else {
                        (vec![y, 0, 0], vec![1, nx, 4])
                    };
                    file.region(path, &start, &count).map_err(|e| e.to_string())
                };
                let lat = read(lat_path, lat_shape)?;
                let lon = read(lon_path, lon_shape)?;
                Some((lat, lon))
            } else {
                None
            };
            for &x in &columns {
                let latitude = latitudes.data[x];
                let longitude = longitudes.data[x];
                if !latitude.is_finite()
                    || !longitude.is_finite()
                    || !(-90.0..=90.0).contains(&latitude)
                {
                    continue;
                }
                let value = values.data[x];
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
                    scan_direction: geolocation
                        .scan_direction
                        .as_ref()
                        .zip(scan_values.as_ref())
                        .map(|(scan, values)| scan.classify(values.data[x], values.fill_value)),
                    footprint: corners.as_ref().and_then(|(lat, lon)| {
                        let mut polygon = [[0.0; 2]; 4];
                        for corner in 0..4 {
                            let index = x * 4 + corner;
                            let latitude = *lat.data.get(index)? as f32;
                            let longitude = *lon.data.get(index)? as f32;
                            if !latitude.is_finite() || !longitude.is_finite() {
                                return None;
                            }
                            polygon[corner] =
                                [((longitude + 180.0).rem_euclid(360.0) - 180.0), latitude];
                        }
                        Some(polygon)
                    }),
                });
            }
        }
    }
    Ok(MapData { points, range })
}

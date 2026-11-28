//! Gridded mosaic subset writer.
use super::*;

pub(super) fn write_mosaic(
    out: &mut netcdf::FileMut,
    products: &[PathBuf],
    choices: &[SubsetVariable],
    bounds: SubsetBounds,
    from: &str,
    through: &str,
    resolution: f64,
    method: MosaicMethod,
) -> Result<usize, String> {
    if !resolution.is_finite() || !(0.05..=10.0).contains(&resolution) {
        return Err("Grid resolution must be between 0.05 and 10 degrees".into());
    }
    let width = (bounds.width() / resolution).ceil() as usize;
    let height = ((bounds.north - bounds.south) / resolution).ceil() as usize;
    let cells = width.checked_mul(height).ok_or("Grid size overflow")?;
    if cells == 0 || cells > 2_000_000 {
        return Err("Grid exceeds two million cells; increase resolution or narrow bounds".into());
    }
    out.add_attribute("mosaic_method", method.label())
        .map_err(err)?;
    out.add_attribute("mosaic_order", "Products are processed in the supplied order; within each product, CF time samples are processed from oldest to newest when available.").map_err(err)?;
    out.add_dimension("latitude", height).map_err(err)?;
    out.add_dimension("longitude", width).map_err(err)?;
    let lat_edges = (0..=height)
        .map(|i| (bounds.south + i as f64 * resolution).min(bounds.north))
        .collect::<Vec<_>>();
    let longitude = |offset: f64| {
        let x = bounds.west + offset;
        if bounds.west == -180.0 && bounds.east == 180.0 && x == 180.0 {
            180.0
        } else {
            (x + 180.0).rem_euclid(360.0) - 180.0
        }
    };
    let lon_edges = (0..=width)
        .map(|i| longitude((i as f64 * resolution).min(bounds.width())))
        .collect::<Vec<_>>();
    let lat = lat_edges
        .windows(2)
        .map(|edge| (edge[0] + edge[1]) * 0.5)
        .collect::<Vec<_>>();
    let lon = (0..width)
        .map(|i| {
            let west = i as f64 * resolution;
            let east = ((i + 1) as f64 * resolution).min(bounds.width());
            longitude((west + east) * 0.5)
        })
        .collect::<Vec<_>>();
    out.add_variable::<f64>("latitude", &["latitude"])
        .map_err(err)?
        .put_values(&lat, ..)
        .map_err(err)?;
    out.add_variable::<f64>("longitude", &["longitude"])
        .map_err(err)?
        .put_values(&lon, ..)
        .map_err(err)?;
    out.variable_mut("latitude")
        .unwrap()
        .put_attribute("units", "degrees_north")
        .map_err(err)?;
    out.variable_mut("longitude")
        .unwrap()
        .put_attribute("units", "degrees_east")
        .map_err(err)?;
    out.add_dimension("nv4", 4).map_err(err)?;
    {
        let mut variable = out
            .add_variable::<f64>("grid_latitude", &["latitude", "longitude"])
            .map_err(err)?;
        variable
            .put_attribute("standard_name", "latitude")
            .map_err(err)?;
        variable
            .put_attribute("units", "degrees_north")
            .map_err(err)?;
        variable
            .put_attribute("bounds", "grid_latitude_bounds")
            .map_err(err)?;
        for (row, &center) in lat.iter().enumerate() {
            variable
                .put_values(&vec![center; width], ([row, 0], [1, width]))
                .map_err(err)?;
        }
    }
    {
        let mut variable = out
            .add_variable::<f64>("grid_longitude", &["latitude", "longitude"])
            .map_err(err)?;
        variable
            .put_attribute("standard_name", "longitude")
            .map_err(err)?;
        variable
            .put_attribute("units", "degrees_east")
            .map_err(err)?;
        variable
            .put_attribute("bounds", "grid_longitude_bounds")
            .map_err(err)?;
        for row in 0..height {
            variable
                .put_values(&lon, ([row, 0], [1, width]))
                .map_err(err)?;
        }
    }
    {
        let mut variable = out
            .add_variable::<f64>("grid_latitude_bounds", &["latitude", "longitude", "nv4"])
            .map_err(err)?;
        for row in 0..height {
            let south = lat_edges[row];
            let north = lat_edges[row + 1];
            let corners = (0..width)
                .flat_map(|_| [south, south, north, north])
                .collect::<Vec<_>>();
            variable
                .put_values(&corners, ([row, 0, 0], [1, width, 4]))
                .map_err(err)?;
        }
    }
    {
        let mut variable = out
            .add_variable::<f64>("grid_longitude_bounds", &["latitude", "longitude", "nv4"])
            .map_err(err)?;
        let corners = (0..width)
            .flat_map(|column| {
                let west = lon_edges[column];
                let east = lon_edges[column + 1];
                [west, east, east, west]
            })
            .collect::<Vec<_>>();
        for row in 0..height {
            variable
                .put_values(&corners, ([row, 0, 0], [1, width, 4]))
                .map_err(err)?;
        }
    }
    let mut used = std::collections::HashSet::new();
    for (variable_index, choice) in choices.iter().enumerate() {
        let channels = if choice.shape.len() == 4 {
            choice.shape[3]
        } else {
            1
        };
        let value_cells = cells.checked_mul(channels).ok_or("Grid size overflow")?;
        if value_cells > 8_000_000 {
            return Err(
                "Grid has too many cells across all channels; increase resolution or narrow bounds"
                    .into(),
            );
        }
        let mut accumulated = vec![0.0; value_cells];
        let mut counts = vec![0_u32; value_cells];
        for path in products {
            let source = netcdf::open(path).map_err(err)?;
            let choice = same_choice(&source, choice)?;
            if choice.shape.len() == 4 && choice.shape[3] != channels {
                return Err("Products have different channel counts".into());
            }
            let (t0, t1) = time_window(&source, &choice, from, through)?;
            let mut time_indices = (t0..t1).collect::<Vec<_>>();
            if matches!(method, MosaicMethod::Newest | MosaicMethod::Oldest)
                && time_indices.len() > 1
            {
                let time_path = choice.time.as_ref().ok_or(
                    "Newest/oldest mosaic needs a CF time coordinate for multi-time products",
                )?;
                let time = source
                    .variable(time_path)
                    .ok_or("Time coordinate missing")?;
                if !attr_string(&time, "units")
                    .to_ascii_lowercase()
                    .contains(" since ")
                {
                    return Err("Newest/oldest mosaic needs CF time units".into());
                }
                let values = time
                    .get_values::<f64, _>((vec![t0], vec![t1 - t0]))
                    .map_err(err)?;
                if values.iter().any(|value| !value.is_finite()) {
                    return Err("Newest/oldest mosaic needs finite time coordinates".into());
                }
                time_indices.sort_by(|&left, &right| {
                    values[left - t0]
                        .total_cmp(&values[right - t0])
                        .then_with(|| left.cmp(&right))
                });
            }
            let src = source.variable(&choice.path).ok_or("Variable missing")?;
            let scale = attr_number(&src, "scale_factor").unwrap_or(1.0);
            let offset = attr_number(&src, "add_offset").unwrap_or(0.0);
            let (rows, cols) = spatial_shape(&choice);
            let mut hit = false;
            let fill = attr_number(&src, "_FillValue");
            let missing = attr_number(&src, "missing_value");
            for &t in &time_indices {
                for r in 0..rows {
                    let chunk_columns = (4096 / channels).max(1);
                    for c in (0..cols).step_by(chunk_columns) {
                        let len = (cols - c).min(chunk_columns);
                        let (lats, lons) = row_coords(&source, &choice, r, c, len)?;
                        let values = data_row(&src, &choice, t, r, c, len)?;
                        for i in 0..len {
                            let la = lats[i];
                            let lo = lons[i];
                            if !bounds.contains(la, lo) {
                                continue;
                            }
                            let y = ((la - bounds.south) / resolution)
                                .floor()
                                .clamp(0.0, (height - 1) as f64)
                                as usize;
                            let x = (bounds.offset(lo) / resolution)
                                .floor()
                                .clamp(0.0, (width - 1) as f64)
                                as usize;
                            for channel in 0..channels {
                                let raw = values[i * channels + channel];
                                if !raw.is_finite() || fill == Some(raw) || missing == Some(raw) {
                                    continue;
                                }
                                let v = raw * scale + offset;
                                if v.is_finite() {
                                    let cell = (y * width + x) * channels + channel;
                                    match method {
                                        MosaicMethod::Mean => accumulated[cell] += v,
                                        MosaicMethod::Newest => accumulated[cell] = v,
                                        MosaicMethod::Oldest if counts[cell] == 0 => {
                                            accumulated[cell] = v
                                        }
                                        MosaicMethod::Minimum
                                            if counts[cell] == 0 || v < accumulated[cell] =>
                                        {
                                            accumulated[cell] = v
                                        }
                                        MosaicMethod::Maximum
                                            if counts[cell] == 0 || v > accumulated[cell] =>
                                        {
                                            accumulated[cell] = v
                                        }
                                        _ => {}
                                    }
                                    counts[cell] = counts[cell].saturating_add(1);
                                    hit = true;
                                }
                            }
                        }
                    }
                }
            }
            if hit {
                used.insert(path);
            }
        }
        let name = choice.path.rsplit('/').next().unwrap_or(&choice.path);
        let values = accumulated
            .into_iter()
            .zip(&counts)
            .map(|(value, &n)| {
                if n == 0 {
                    f64::NAN
                } else if method == MosaicMethod::Mean {
                    value / n as f64
                } else {
                    value
                }
            })
            .collect::<Vec<_>>();
        let channel_name = format!("channel_{variable_index}");
        let dims = if choice.shape.len() == 4 {
            if out.dimension("time").is_none() {
                out.add_dimension("time", 1).map_err(err)?;
            }
            out.add_dimension(&channel_name, channels).map_err(err)?;
            let indexes = (0..channels).map(|i| i as i64).collect::<Vec<_>>();
            out.add_variable::<i64>(&channel_name, &[&channel_name])
                .map_err(err)?
                .put_values(&indexes, ..)
                .map_err(err)?;
            vec!["time", "latitude", "longitude", channel_name.as_str()]
        } else {
            vec!["latitude", "longitude"]
        };
        let mut dst = out.add_variable::<f64>(name, &dims).map_err(err)?;
        dst.set_fill_value(f64::NAN).map_err(err)?;
        dst.put_values(&values, ..).map_err(err)?;
        if let Some(source) = products.first().and_then(|p| netcdf::open(p).ok()) {
            if let Some(src) = source.variable(&choice.path) {
                copy_attrs(
                    &src,
                    &mut dst,
                    &[
                        "_FillValue",
                        "missing_value",
                        "scale_factor",
                        "add_offset",
                        "coordinates",
                    ],
                )?;
            }
        }
        dst.put_attribute(
            "coordinates",
            if choice.shape.len() == 4 {
                format!("grid_latitude grid_longitude {channel_name}")
            } else {
                "grid_latitude grid_longitude".to_owned()
            },
        )
        .map_err(err)?;
        drop(dst);
        let mut count_variable = out
            .add_variable::<u32>(&format!("{name}_count"), &dims)
            .map_err(err)?;
        count_variable
            .put_attribute(
                "coordinates",
                if choice.shape.len() == 4 {
                    format!("grid_latitude grid_longitude {channel_name}")
                } else {
                    "grid_latitude grid_longitude".to_owned()
                },
            )
            .map_err(err)?;
        count_variable.put_values(&counts, ..).map_err(err)?;
    }
    Ok(used.len())
}

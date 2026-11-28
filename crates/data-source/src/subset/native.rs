//! Native-preserving subset writer.
use super::*;

pub(super) fn write_native_product(
    out: &mut netcdf::FileMut,
    source: &netcdf::File,
    source_path: &Path,
    choices: &[SubsetVariable],
    geolocation_rules: &[GeolocationRule],
    bounds: SubsetBounds,
    from: &str,
    through: &str,
    product_index: usize,
) -> Result<bool, String> {
    let first = same_choice(source, &choices[0])?;
    let source_geolocation = File::open(source_path)
        .map_err(err)?
        .geolocation_with_rules(&first.path, geolocation_rules)
        .map_err(err)?;
    let Some(window) = spatial_window(source, &first, bounds)? else {
        return Ok(false);
    };
    let (r0, r1) = (window.row_start, window.row_end);
    let (t0, t1) = time_window(source, &first, from, through)?;
    let prefix = format!("product_{product_index:04}");
    out.add_group(&prefix).map_err(err)?;
    for (name, len) in [("row", r1 - r0), ("column", window.width)] {
        out.add_dimension(&format!("{prefix}/{name}"), len)
            .map_err(err)?;
    }
    if first.shape.len() >= 3 {
        out.add_dimension(&format!("{prefix}/time"), t1 - t0)
            .map_err(err)?;
    }
    if first.shape.len() == 4 {
        out.add_dimension(&format!("{prefix}/channel"), first.shape[3])
            .map_err(err)?;
        let channel = (0..first.shape[3]).map(|i| i as i64).collect::<Vec<_>>();
        out.add_variable::<i64>(&format!("{prefix}/channel"), &["channel"])
            .map_err(err)?
            .put_values(&channel, ..)
            .map_err(err)?;
    }
    if source_geolocation
        .as_ref()
        .is_some_and(|geo| geo.latitude_bounds.is_some() || geo.longitude_bounds.is_some())
    {
        out.add_dimension(&format!("{prefix}/nv4"), 4)
            .map_err(err)?;
    }
    let dims = if first.shape.len() == 4 {
        vec!["time", "row", "column", "channel"]
    } else if first.shape.len() == 3 {
        vec!["time", "row", "column"]
    } else {
        vec!["row", "column"]
    };
    for (name, path) in [
        ("latitude", &first.latitude),
        ("longitude", &first.longitude),
    ] {
        let src = source.variable(path).ok_or("Coordinate missing")?;
        let coord_dims = if first.regular_grid {
            if name == "latitude" {
                vec!["row"]
            } else {
                vec!["column"]
            }
        } else {
            vec!["row", "column"]
        };
        let mut dst = out
            .add_variable::<f64>(&format!("{prefix}/{name}"), &coord_dims)
            .map_err(err)?;
        copy_attrs(&src, &mut dst, &["_FillValue"])?;
        if !first.regular_grid {
            let bound = source_geolocation.as_ref().and_then(|geo| {
                if name == "latitude" {
                    geo.latitude_bounds.as_ref()
                } else {
                    geo.longitude_bounds.as_ref()
                }
            });
            if bound.is_some() {
                dst.put_attribute("bounds", format!("{name}_bounds"))
                    .map_err(err)?;
            }
        }
        if first.regular_grid {
            if name == "latitude" {
                for i in (r0..r1).step_by(4096) {
                    let len = (r1 - i).min(4096);
                    let values = src
                        .get_values::<f64, _>((vec![i], vec![len]))
                        .map_err(err)?;
                    dst.put_values(&values, ([i - r0], [len])).map_err(err)?;
                }
            } else {
                for run in &window.columns {
                    for offset in (0..run.count).step_by(4096) {
                        let len = (run.count - offset).min(4096);
                        let values = src
                            .get_values::<f64, _>((vec![run.source + offset], vec![len]))
                            .map_err(err)?;
                        let values = values
                            .into_iter()
                            .map(|longitude| {
                                if bounds.west == -180.0 && bounds.east == 180.0 {
                                    (longitude + 180.0).rem_euclid(360.0) - 180.0
                                } else {
                                    bounds.longitude(longitude)
                                }
                            })
                            .collect::<Vec<_>>();
                        dst.put_values(&values, ([run.output + offset], [len]))
                            .map_err(err)?;
                    }
                }
            }
        } else {
            for r in r0..r1 {
                for run in &window.columns {
                    for offset in (0..run.count).step_by(4096) {
                        let len = (run.count - offset).min(4096);
                        let c = run.source + offset;
                        let (start, count) = if src.dimensions().len() == 3 {
                            (vec![0, r, c], vec![1, 1, len])
                        } else {
                            (vec![r, c], vec![1, len])
                        };
                        let row = src.get_values::<f64, _>((start, count)).map_err(err)?;
                        dst.put_values(&row, ([r - r0, run.output + offset], [1, len]))
                            .map_err(err)?;
                    }
                }
            }
        }
    }
    if let Some(geo) = &source_geolocation {
        if let Some(scan) = geo
            .scan_direction
            .as_ref()
            .filter(|scan| scan.constant.is_some())
        {
            let code = match scan.constant.unwrap() {
                super::ScanDirectionClass::Forward => 1.0,
                super::ScanDirectionClass::Backward => 2.0,
                super::ScanDirectionClass::Other => f64::NAN,
            };
            let mut dst = out
                .add_variable::<f64>(&format!("{prefix}/scan_direction"), &["row", "column"])
                .map_err(err)?;
            for row in 0..r1 - r0 {
                for run in &window.columns {
                    let values = vec![code; run.count];
                    dst.put_values(&values, ([row, run.output], [1, run.count]))
                        .map_err(err)?;
                }
            }
        }
        for (name, path) in [
            ("latitude_bounds", geo.latitude_bounds.as_ref()),
            ("longitude_bounds", geo.longitude_bounds.as_ref()),
            (
                "scan_direction",
                geo.scan_direction
                    .as_ref()
                    .filter(|scan| scan.constant.is_none())
                    .map(|scan| &scan.path),
            ),
        ] {
            let Some(path) = path else { continue };
            let src = source
                .variable(path.trim_start_matches('/'))
                .ok_or("Geolocation auxiliary variable missing")?;
            let shape = src.dimensions().iter().map(|d| d.len()).collect::<Vec<_>>();
            let bounds_variable = name.ends_with("_bounds");
            let (rows, cols) = spatial_shape(&first);
            if shape.as_slice() != [rows, cols]
                && shape.as_slice() != [1, rows, cols]
                && !(bounds_variable && shape.as_slice() == [rows, cols, 4])
                && !(bounds_variable && shape.as_slice() == [1, rows, cols, 4])
            {
                continue;
            }
            let dims = if bounds_variable {
                vec!["row", "column", "nv4"]
            } else {
                vec!["row", "column"]
            };
            let mut dst = out
                .add_variable::<f64>(&format!("{prefix}/{name}"), &dims)
                .map_err(err)?;
            copy_attrs(&src, &mut dst, &["_FillValue"])?;
            let corners = if bounds_variable { 4 } else { 1 };
            for r in r0..r1 {
                for run in &window.columns {
                    for offset in (0..run.count).step_by((4096 / corners).max(1)) {
                        let len = (run.count - offset).min((4096 / corners).max(1));
                        let c = run.source + offset;
                        let (start, count) = match shape.len() {
                            4 => (vec![0, r, c, 0], vec![1, 1, len, 4]),
                            3 if bounds_variable => (vec![r, c, 0], vec![1, len, 4]),
                            3 => (vec![0, r, c], vec![1, 1, len]),
                            _ => (vec![r, c], vec![1, len]),
                        };
                        let values = src.get_values::<f64, _>((start, count)).map_err(err)?;
                        let (start, count) = if bounds_variable {
                            (vec![r - r0, run.output + offset, 0], vec![1, len, 4])
                        } else {
                            (vec![r - r0, run.output + offset], vec![1, len])
                        };
                        dst.put_values(&values, (start, count)).map_err(err)?;
                    }
                }
            }
        }
    }
    if let Some(path) = &first.time {
        if first.shape.len() >= 3 {
            let src = source.variable(path).ok_or("Time coordinate missing")?;
            let mut dst = out
                .add_variable::<f64>(&format!("{prefix}/time"), &["time"])
                .map_err(err)?;
            copy_attrs(&src, &mut dst, &["_FillValue"])?;
            for t in (t0..t1).step_by(4096) {
                let len = (t1 - t).min(4096);
                let values = src
                    .get_values::<f64, _>((vec![t], vec![len]))
                    .map_err(err)?;
                dst.put_values(&values, ([t - t0], [len])).map_err(err)?;
            }
        }
    }
    for (variable_index, selected) in choices.iter().enumerate() {
        let choice = same_choice(source, selected)?;
        if choice.latitude != first.latitude
            || choice.longitude != first.longitude
            || choice.shape != first.shape
        {
            return Err("Selected variables must share coordinates and shape".into());
        }
        let src = source.variable(&choice.path).ok_or("Variable missing")?;
        let name = choice.path.rsplit('/').next().unwrap_or(&choice.path);
        let name = if ["latitude", "longitude", "time"].contains(&name) {
            format!("field_{variable_index}_{name}")
        } else {
            name.to_owned()
        };
        let mut dst = out
            .add_variable::<f64>(&format!("{prefix}/{name}"), &dims)
            .map_err(err)?;
        dst.set_fill_value(f64::NAN).map_err(err)?;
        copy_attrs(
            &src,
            &mut dst,
            &["_FillValue", "missing_value", "coordinates"],
        )?;
        dst.put_attribute(
            "coordinates",
            if first.shape.len() == 4 && first.time.is_some() {
                "time latitude longitude channel"
            } else if first.shape.len() == 4 {
                "latitude longitude channel"
            } else if first.time.is_some() {
                "time latitude longitude"
            } else {
                "latitude longitude"
            },
        )
        .map_err(err)?;
        if let Some(scan) = source_geolocation
            .as_ref()
            .and_then(|geo| geo.scan_direction.as_ref())
        {
            dst.put_attribute("scan_direction", "scan_direction")
                .map_err(err)?;
            let codes = |values: &[i64]| {
                values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            };
            dst.put_attribute("scan_forward_values", codes(&scan.forward_values))
                .map_err(err)?;
            dst.put_attribute("scan_backward_values", codes(&scan.backward_values))
                .map_err(err)?;
            dst.put_attribute(
                "scan_default_filter",
                match scan.default_filter {
                    super::ScanFilter::Forward => "forward",
                    super::ScanFilter::Backward => "backward",
                    super::ScanFilter::Both => "both",
                },
            )
            .map_err(err)?;
        }
        let fill = attr_number(&src, "_FillValue");
        let missing = attr_number(&src, "missing_value");
        let channels = if choice.shape.len() == 4 {
            choice.shape[3]
        } else {
            1
        };
        let chunk_columns = (4096 / channels).max(1);
        for t in t0..t1 {
            for r in r0..r1 {
                for run in &window.columns {
                    for offset in (0..run.count).step_by(chunk_columns) {
                        let c = run.source + offset;
                        let len = (run.count - offset).min(chunk_columns);
                        let (lats, lons) = row_coords(source, &choice, r, c, len)?;
                        let mut row = data_row(&src, &choice, t, r, c, len)?;
                        for i in 0..len {
                            for value in &mut row[i * channels..(i + 1) * channels] {
                                if !bounds.contains(lats[i], lons[i])
                                    || !value.is_finite()
                                    || fill == Some(*value)
                                    || missing == Some(*value)
                                {
                                    *value = f64::NAN;
                                }
                            }
                        }
                        let (start, count) = if choice.shape.len() == 4 {
                            (
                                vec![t - t0, r - r0, run.output + offset, 0],
                                vec![1, 1, len, channels],
                            )
                        } else if choice.shape.len() == 3 {
                            (vec![t - t0, r - r0, run.output + offset], vec![1, 1, len])
                        } else {
                            (vec![r - r0, run.output + offset], vec![1, len])
                        };
                        dst.put_values(&row, (start, count)).map_err(err)?;
                    }
                }
            }
        }
    }
    Ok(true)
}

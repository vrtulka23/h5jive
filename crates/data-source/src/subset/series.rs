//! Combined time-series subset writer.
use super::*;

struct SeriesProduct {
    path: PathBuf,
    source_index: usize,
    window: SpatialWindow,
    times: Vec<(usize, Option<f64>)>,
    geolocation: Option<Geolocation>,
}

pub(super) fn series_row_coords(
    file: &netcdf::File,
    choice: &SubsetVariable,
    time: usize,
    row: usize,
    column: usize,
    len: usize,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    if choice.regular_grid {
        return row_coords(file, choice, row, column, len);
    }
    let read = |path: &str| -> Result<Vec<f64>, String> {
        let variable = file.variable(path).ok_or("Coordinate variable missing")?;
        let (start, count) = if variable.dimensions().len() == 3 {
            let axis = if variable.dimensions()[0].len() > 1 {
                time
            } else {
                0
            };
            (vec![axis, row, column], vec![1, 1, len])
        } else {
            (vec![row, column], vec![1, len])
        };
        variable.get_values::<f64, _>((start, count)).map_err(err)
    };
    Ok((read(&choice.latitude)?, read(&choice.longitude)?))
}

pub(super) fn series_aux_row(
    file: &netcdf::File,
    path: &str,
    time: usize,
    row: usize,
    column: usize,
    len: usize,
    bounds: bool,
) -> Result<Vec<f64>, String> {
    let variable = file
        .variable(path.trim_start_matches('/'))
        .ok_or("Swath auxiliary coordinate missing")?;
    let (start, shape) = match (bounds, variable.dimensions().len()) {
        (true, 4) => (
            vec![
                if variable.dimensions()[0].len() > 1 {
                    time
                } else {
                    0
                },
                row,
                column,
                0,
            ],
            vec![1, 1, len, 4],
        ),
        (true, 3) => (vec![row, column, 0], vec![1, len, 4]),
        (false, 3) => (
            vec![
                if variable.dimensions()[0].len() > 1 {
                    time
                } else {
                    0
                },
                row,
                column,
            ],
            vec![1, 1, len],
        ),
        (false, 2) => (vec![row, column], vec![1, len]),
        _ => return Err("Unsupported swath auxiliary coordinate shape".into()),
    };
    variable.get_values::<f64, _>((start, shape)).map_err(err)
}

pub(super) fn series_axis(
    file: &netcdf::File,
    choice: &SubsetVariable,
    window: &SpatialWindow,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let lat = file
        .variable(&choice.latitude)
        .ok_or("Latitude coordinate missing")?;
    let lon = file
        .variable(&choice.longitude)
        .ok_or("Longitude coordinate missing")?;
    let rows = window.row_end - window.row_start;
    let latitudes = lat
        .get_values::<f64, _>((vec![window.row_start], vec![rows]))
        .map_err(err)?;
    let mut longitudes = vec![0.0; window.width];
    for run in &window.columns {
        for offset in (0..run.count).step_by(4096) {
            let count = (run.count - offset).min(4096);
            let values = lon
                .get_values::<f64, _>((vec![run.source + offset], vec![count]))
                .map_err(err)?;
            for (j, value) in values.into_iter().enumerate() {
                longitudes[run.output + offset + j] = value;
            }
        }
    }
    Ok((latitudes, longitudes))
}

pub(super) fn series_time_values(
    file: &netcdf::File,
    choice: &SubsetVariable,
    from: &str,
    through: &str,
) -> Result<Vec<(usize, Option<f64>)>, String> {
    if let Some(path) = &choice.time {
        let variable = file.variable(path).ok_or("Time coordinate missing")?;
        let calendar = attr_string(&variable, "calendar").to_ascii_lowercase();
        if !calendar.is_empty()
            && !["standard", "gregorian", "proleptic_gregorian"].contains(&calendar.as_str())
        {
            return Err(format!("Unsupported CF calendar: {calendar}"));
        }
        let units = attr_string(&variable, "units").to_ascii_lowercase();
        let (unit, origin) = units
            .split_once(" since ")
            .ok_or("Combined time series requires CF time units")?;
        let origin = parse_origin_seconds(origin).ok_or("Could not parse CF time origin")?;
        let factor = if unit.starts_with("day") {
            86400.0
        } else if unit.starts_with("hour") {
            3600.0
        } else if unit.starts_with("minute") {
            60.0
        } else if unit.starts_with("second") {
            1.0
        } else {
            return Err(format!("Unsupported time units: {unit}"));
        };
        let (start, end) = time_window(file, choice, from, through)?;
        let mut result = Vec::with_capacity(end - start);
        for chunk in (start..end).step_by(4096) {
            let len = (end - chunk).min(4096);
            for (offset, value) in variable
                .get_values::<f64, _>((vec![chunk], vec![len]))
                .map_err(err)?
                .into_iter()
                .enumerate()
            {
                let time = origin + value * factor;
                if !time.is_finite()
                    || result
                        .last()
                        .is_some_and(|(_, prior): &(usize, Option<f64>)| {
                            prior.is_some_and(|prior| time <= prior)
                        })
                {
                    return Err("Time coordinates must be finite and strictly increasing within each product".into());
                }
                result.push((chunk + offset, Some(time)));
            }
        }
        Ok(result)
    } else {
        let count = if choice.shape.len() == 2 {
            1
        } else {
            choice.shape[0]
        };
        Ok((0..count).map(|index| (index, None)).collect())
    }
}

pub(super) fn write_time_series(
    out: &mut netcdf::FileMut,
    request: &SubsetRequest,
) -> Result<usize, String> {
    let group = request.series_group.trim();
    if !group.is_empty() && !netcdf_name(group) {
        return Err("Group name must contain letters, digits, or underscores and start with a letter or underscore".into());
    }
    let prefix = if group.is_empty() {
        String::new()
    } else {
        format!("{group}/")
    };
    let first_file = netcdf::open(&request.products[0]).map_err(err)?;
    let first = same_choice(&first_file, &request.variables[0])?;
    let regular_grid = first.regular_grid;
    let has_time = first.time.is_some();
    let source_dimension = if has_time {
        Some(
            first_file
                .variable(&first.path)
                .ok_or("Variable missing")?
                .dimensions()[0]
                .name(),
        )
    } else {
        None
    };
    let dimension = source_dimension
        .as_deref()
        .unwrap_or_else(|| request.series_dimension.trim());
    if !netcdf_name(dimension)
        || (!has_time && dimension == "time")
        || ["latitude", "longitude", "row", "column", "channel"].contains(&dimension)
    {
        return Err(
            "Enter a distinct first dimension name using letters, digits, or underscores".into(),
        );
    }
    let rank = first.shape.len();
    let reference_calendar = first
        .time
        .as_ref()
        .and_then(|path| first_file.variable(path))
        .map(|variable| attr_string(&variable, "calendar").to_ascii_lowercase());
    let channels = if rank == 4 { first.shape[3] } else { 1 };
    let mut plans = Vec::new();
    let mut expected_axis: Option<(Vec<f64>, Vec<f64>)> = None;
    let mut max_rows = 0;
    let mut max_columns = 0;
    let mut expected_geolocation: Option<Geolocation> = None;
    for (source_index, path) in request.products.iter().enumerate() {
        let file = netcdf::open(path).map_err(err)?;
        let choice = same_choice(&file, &first)?;
        if choice.regular_grid != regular_grid
            || choice.shape.len() != rank
            || choice.time.is_some() != has_time
            || (rank == 4 && choice.shape[3] != channels)
        {
            return Err(format!(
                "{} has incompatible rank, channel count, or time coordinate",
                path.display()
            ));
        }
        if has_time {
            let calendar = choice
                .time
                .as_ref()
                .and_then(|path| file.variable(path))
                .map(|variable| attr_string(&variable, "calendar").to_ascii_lowercase());
            if calendar != reference_calendar {
                return Err(format!("{} uses a different CF calendar", path.display()));
            }
        }
        if !regular_grid
            && [&choice.latitude, &choice.longitude]
                .iter()
                .any(|coordinate| {
                    file.variable(coordinate).is_some_and(|variable| {
                        variable.dimensions().len() == 3 && variable.dimensions()[0].len() > 1
                    })
                })
        {
            return Err(format!(
                "{} has coordinates that vary within a product; use Native products for this geometry",
                path.display()
            ));
        }
        let Some(window) = spatial_window(&file, &choice, request.bounds)? else {
            continue;
        };
        max_rows = max_rows.max(window.row_end - window.row_start);
        max_columns = max_columns.max(window.width);
        if regular_grid {
            let axis = series_axis(&file, &choice, &window)?;
            if let Some(expected) = &expected_axis {
                if &axis != expected {
                    return Err(format!(
                        "{} has a different cropped latitude/longitude grid",
                        path.display()
                    ));
                }
            } else {
                expected_axis = Some(axis);
            }
        } else {
            let geo = File::open(path)
                .map_err(err)?
                .geolocation_with_rules(&choice.path, &request.geolocation_rules)
                .map_err(err)?;
            if let (Some(reference), Some(current)) = (&expected_geolocation, &geo) {
                if reference.latitude_bounds.is_some() != current.latitude_bounds.is_some()
                    || reference.longitude_bounds.is_some() != current.longitude_bounds.is_some()
                    || reference
                        .scan_direction
                        .as_ref()
                        .map(|scan| (&scan.forward_values, &scan.backward_values))
                        != current
                            .scan_direction
                            .as_ref()
                            .map(|scan| (&scan.forward_values, &scan.backward_values))
                {
                    return Err(format!(
                        "{} has incompatible swath bounds or scan direction",
                        path.display()
                    ));
                }
            } else if expected_geolocation.is_some() != geo.is_some() && !plans.is_empty() {
                return Err(format!(
                    "{} has incompatible swath geolocation metadata",
                    path.display()
                ));
            }
            if plans.is_empty() {
                expected_geolocation = geo.clone();
            }
            // The per-product geolocation paths are retained for writing below.
            let times =
                series_time_values(&file, &choice, &request.from_date, &request.through_date)?;
            if times.is_empty() {
                continue;
            }
            for selected in &request.variables {
                let adjusted = same_choice(&file, selected)?;
                let spatial_start = if rank >= 3 { 1 } else { 0 };
                if adjusted.shape.len() != rank
                    || (rank >= 3 && adjusted.shape[0] != choice.shape[0])
                    || adjusted.shape[spatial_start..] != choice.shape[spatial_start..]
                    || adjusted.latitude != choice.latitude
                    || adjusted.longitude != choice.longitude
                    || adjusted.time != choice.time
                {
                    return Err(format!(
                        "{} has incompatible selected variables",
                        path.display()
                    ));
                }
                let source = file.variable(&selected.path).ok_or("Variable missing")?;
                let reference = first_file
                    .variable(&selected.path)
                    .ok_or("Reference variable missing")?;
                for key in ["units", "standard_name"] {
                    if attr_string(&source, key) != attr_string(&reference, key) {
                        return Err(format!(
                            "{} has different {key} for {}",
                            path.display(),
                            selected.path
                        ));
                    }
                }
            }
            plans.push(SeriesProduct {
                path: path.clone(),
                source_index,
                window,
                times,
                geolocation: geo,
            });
            continue;
        }
        for selected in &request.variables {
            let adjusted = same_choice(&file, selected)?;
            if adjusted.shape.len() != rank
                || (rank >= 3 && adjusted.shape[0] != choice.shape[0])
                || adjusted.time != choice.time
                || adjusted.shape[rank - 2..] != choice.shape[rank - 2..] && rank != 4
                || adjusted.latitude != choice.latitude
                || adjusted.longitude != choice.longitude
                || (rank == 4 && adjusted.shape[1..] != choice.shape[1..])
            {
                return Err(format!(
                    "{} has incompatible selected variables",
                    path.display()
                ));
            }
            let source = file.variable(&selected.path).ok_or("Variable missing")?;
            let reference = first_file
                .variable(&selected.path)
                .ok_or("Reference variable missing")?;
            for key in ["units", "standard_name"] {
                if attr_string(&source, key) != attr_string(&reference, key) {
                    return Err(format!(
                        "{} has different {key} for {}",
                        path.display(),
                        selected.path
                    ));
                }
            }
        }
        let times = series_time_values(&file, &choice, &request.from_date, &request.through_date)?;
        if !times.is_empty() {
            plans.push(SeriesProduct {
                path: path.clone(),
                source_index,
                window,
                times,
                geolocation: None,
            });
        }
    }
    if plans.is_empty() {
        return Err("No selected product has values inside the bounds and time range".into());
    }
    if has_time {
        plans.sort_by(|a, b| a.times[0].1.unwrap().total_cmp(&b.times[0].1.unwrap()));
        let mut previous = None;
        for plan in &plans {
            if previous.is_some_and(|last| plan.times[0].1.unwrap() <= last) {
                return Err("Product time ranges overlap or contain duplicate timestamps".into());
            }
            previous = plan.times.last().and_then(|(_, time)| *time);
        }
    }
    let count = plans.iter().map(|plan| plan.times.len()).sum::<usize>();
    let (rows, columns) = if let Some((latitudes, longitudes)) = &expected_axis {
        (latitudes.len(), longitudes.len())
    } else {
        (max_rows, max_columns)
    };
    if !group.is_empty() {
        out.add_group(group).map_err(err)?;
    }
    for (name, size) in [(dimension, count), ("row", rows), ("column", columns)] {
        out.add_dimension(&format!("{prefix}{name}"), size)
            .map_err(err)?;
    }
    if rank == 4 {
        out.add_dimension(&format!("{prefix}channel"), channels)
            .map_err(err)?;
    }
    if expected_geolocation
        .as_ref()
        .is_some_and(|geo| geo.latitude_bounds.is_some() || geo.longitude_bounds.is_some())
    {
        out.add_dimension(&format!("{prefix}nv4"), 4).map_err(err)?;
    }
    for (name, axis) in [("latitude", "row"), ("longitude", "column")] {
        let source = first_file
            .variable(if name == "latitude" {
                &first.latitude
            } else {
                &first.longitude
            })
            .ok_or("Coordinate missing")?;
        let dims = if regular_grid {
            vec![axis]
        } else {
            vec![dimension, "row", "column"]
        };
        let mut target = out
            .add_variable::<f64>(&format!("{prefix}{name}"), &dims)
            .map_err(err)?;
        if !regular_grid {
            target.set_fill_value(f64::NAN).map_err(err)?;
        }
        copy_attrs(&source, &mut target, &["_FillValue"])?;
        if let Some((latitudes, longitudes)) = &expected_axis {
            target
                .put_values(
                    if name == "latitude" {
                        latitudes
                    } else {
                        longitudes
                    },
                    ..,
                )
                .map_err(err)?;
        } else if expected_geolocation.as_ref().is_some_and(|geo| {
            if name == "latitude" {
                geo.latitude_bounds.is_some()
            } else {
                geo.longitude_bounds.is_some()
            }
        }) {
            target
                .put_attribute("bounds", format!("{name}_bounds"))
                .map_err(err)?;
        }
    }
    if let Some(geo) = &expected_geolocation {
        for (name, present) in [
            ("latitude_bounds", geo.latitude_bounds.is_some()),
            ("longitude_bounds", geo.longitude_bounds.is_some()),
            ("scan_direction", geo.scan_direction.is_some()),
        ] {
            if !present {
                continue;
            }
            let dims = if name.ends_with("_bounds") {
                vec![dimension, "row", "column", "nv4"]
            } else {
                vec![dimension, "row", "column"]
            };
            let mut target = out
                .add_variable::<f64>(&format!("{prefix}{name}"), &dims)
                .map_err(err)?;
            target.set_fill_value(f64::NAN).map_err(err)?;
        }
    }
    if has_time {
        let mut target = out
            .add_variable::<f64>(&format!("{prefix}{dimension}"), &[dimension])
            .map_err(err)?;
        target.put_attribute("standard_name", "time").map_err(err)?;
        target
            .put_attribute("units", "seconds since 1970-01-01 00:00:00")
            .map_err(err)?;
        target
            .put_attribute(
                "calendar",
                if reference_calendar.as_deref() == Some("proleptic_gregorian") {
                    "proleptic_gregorian"
                } else {
                    "standard"
                },
            )
            .map_err(err)?;
        let values = plans
            .iter()
            .flat_map(|plan| plan.times.iter().map(|(_, time)| time.unwrap()))
            .collect::<Vec<_>>();
        target.put_values(&values, ..).map_err(err)?;
    } else {
        out.add_variable::<i64>(&format!("{prefix}{dimension}"), &[dimension])
            .map_err(err)?
            .put_values(&(0..count as i64).collect::<Vec<_>>(), ..)
            .map_err(err)?;
    }
    if rank == 4 {
        out.add_variable::<i64>(&format!("{prefix}channel"), &["channel"])
            .map_err(err)?
            .put_values(&(0..channels as i64).collect::<Vec<_>>(), ..)
            .map_err(err)?;
    }
    let dimensions = if rank == 4 {
        vec![dimension, "row", "column", "channel"]
    } else {
        vec![dimension, "row", "column"]
    };
    for selected in &request.variables {
        let name = selected.path.rsplit('/').next().unwrap_or(&selected.path);
        if [
            dimension,
            "latitude",
            "longitude",
            "channel",
            "row",
            "column",
            "source_product",
            "row_count",
            "column_count",
        ]
        .contains(&name)
        {
            return Err(format!(
                "Variable name {name} conflicts with an output coordinate"
            ));
        }
        let source = first_file
            .variable(&selected.path)
            .ok_or("Variable missing")?;
        let mut target = out
            .add_variable::<f64>(&format!("{prefix}{name}"), &dimensions)
            .map_err(err)?;
        target.set_fill_value(f64::NAN).map_err(err)?;
        copy_attrs(
            &source,
            &mut target,
            &["_FillValue", "missing_value", "coordinates"],
        )?;
        target
            .put_attribute(
                "coordinates",
                if rank == 4 {
                    format!("{dimension} latitude longitude channel")
                } else {
                    format!("{dimension} latitude longitude")
                },
            )
            .map_err(err)?;
        if let Some(scan) = expected_geolocation
            .as_ref()
            .and_then(|geo| geo.scan_direction.as_ref())
        {
            target
                .put_attribute("scan_direction", "scan_direction")
                .map_err(err)?;
            let codes = |values: &[i64]| {
                values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            };
            target
                .put_attribute("scan_forward_values", codes(&scan.forward_values))
                .map_err(err)?;
            target
                .put_attribute("scan_backward_values", codes(&scan.backward_values))
                .map_err(err)?;
            target
                .put_attribute(
                    "scan_default_filter",
                    match scan.default_filter {
                        super::ScanFilter::Forward => "forward",
                        super::ScanFilter::Backward => "backward",
                        super::ScanFilter::Both => "both",
                    },
                )
                .map_err(err)?;
        }
    }
    let mut source_product = out
        .add_variable::<i32>(&format!("{prefix}source_product"), &[dimension])
        .map_err(err)?;
    source_product
        .put_attribute(
            "long_name",
            "index of source product in the selected product list",
        )
        .map_err(err)?;
    let source_indexes = plans
        .iter()
        .flat_map(|plan| std::iter::repeat_n(plan.source_index as i32, plan.times.len()))
        .collect::<Vec<_>>();
    source_product
        .put_values(&source_indexes, ..)
        .map_err(err)?;
    if !regular_grid {
        for (name, sizes) in [
            (
                "row_count",
                plans
                    .iter()
                    .flat_map(|plan| {
                        std::iter::repeat_n(
                            (plan.window.row_end - plan.window.row_start) as i32,
                            plan.times.len(),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            (
                "column_count",
                plans
                    .iter()
                    .flat_map(|plan| {
                        std::iter::repeat_n(plan.window.width as i32, plan.times.len())
                    })
                    .collect::<Vec<_>>(),
            ),
        ] {
            out.add_variable::<i32>(&format!("{prefix}{name}"), &[dimension])
                .map_err(err)?
                .put_values(&sizes, ..)
                .map_err(err)?;
        }
    }
    let mut output_index = 0;
    for plan in &plans {
        let file = netcdf::open(&plan.path).map_err(err)?;
        out.add_attribute(
            &format!("{prefix}source_file_{:04}", plan.source_index),
            plan.path.to_string_lossy().as_ref(),
        )
        .map_err(err)?;
        for &(source_time, _) in &plan.times {
            if !regular_grid {
                let choice = same_choice(&file, &request.variables[0])?;
                for row in plan.window.row_start..plan.window.row_end {
                    for run in &plan.window.columns {
                        for offset in (0..run.count).step_by(1024) {
                            let len = (run.count - offset).min(1024);
                            let column = run.source + offset;
                            let start = [
                                output_index,
                                row - plan.window.row_start,
                                run.output + offset,
                            ];
                            let (latitudes, longitudes) =
                                series_row_coords(&file, &choice, source_time, row, column, len)?;
                            for (name, values) in
                                [("latitude", latitudes), ("longitude", longitudes)]
                            {
                                out.variable_mut(&format!("{prefix}{name}"))
                                    .ok_or("Output coordinate missing")?
                                    .put_values(&values, (start, [1, 1, len]))
                                    .map_err(err)?;
                            }
                            if let Some(geo) = &plan.geolocation {
                                for (name, path, bounds) in [
                                    ("latitude_bounds", geo.latitude_bounds.as_deref(), true),
                                    ("longitude_bounds", geo.longitude_bounds.as_deref(), true),
                                    (
                                        "scan_direction",
                                        geo.scan_direction.as_ref().map(|scan| scan.path.as_str()),
                                        false,
                                    ),
                                ] {
                                    let Some(path) = path else { continue };
                                    let values = if name == "scan_direction" && path.is_empty() {
                                        let code = match geo
                                            .scan_direction
                                            .as_ref()
                                            .and_then(|scan| scan.constant)
                                        {
                                            Some(super::ScanDirectionClass::Forward) => 1.0,
                                            Some(super::ScanDirectionClass::Backward) => 2.0,
                                            _ => f64::NAN,
                                        };
                                        vec![code; len]
                                    } else {
                                        series_aux_row(
                                            &file,
                                            path,
                                            source_time,
                                            row,
                                            column,
                                            len,
                                            bounds,
                                        )?
                                    };
                                    let (start, shape) = if bounds {
                                        (
                                            vec![
                                                output_index,
                                                row - plan.window.row_start,
                                                run.output + offset,
                                                0,
                                            ],
                                            vec![1, 1, len, 4],
                                        )
                                    } else {
                                        (start.to_vec(), vec![1, 1, len])
                                    };
                                    out.variable_mut(&format!("{prefix}{name}"))
                                        .ok_or("Output swath coordinate missing")?
                                        .put_values(&values, (start, shape))
                                        .map_err(err)?;
                                }
                            }
                        }
                    }
                }
            }
            for selected in &request.variables {
                let choice = same_choice(&file, selected)?;
                let source = file.variable(&selected.path).ok_or("Variable missing")?;
                let name = selected.path.rsplit('/').next().unwrap_or(&selected.path);
                let mut target = out
                    .variable_mut(&format!("{prefix}{name}"))
                    .ok_or("Output variable missing")?;
                let fill = attr_number(&source, "_FillValue");
                let missing = attr_number(&source, "missing_value");
                let chunk_columns = (4096 / channels).max(1);
                for row in plan.window.row_start..plan.window.row_end {
                    for run in &plan.window.columns {
                        for offset in (0..run.count).step_by(chunk_columns) {
                            let len = (run.count - offset).min(chunk_columns);
                            let column = run.source + offset;
                            let (lats, lons) =
                                series_row_coords(&file, &choice, source_time, row, column, len)?;
                            let mut values =
                                data_row(&source, &choice, source_time, row, column, len)?;
                            for i in 0..len {
                                for value in &mut values[i * channels..(i + 1) * channels] {
                                    if !request.bounds.contains(lats[i], lons[i])
                                        || !value.is_finite()
                                        || fill == Some(*value)
                                        || missing == Some(*value)
                                    {
                                        *value = f64::NAN;
                                    }
                                }
                            }
                            let (start, shape) = if rank == 4 {
                                (
                                    vec![
                                        output_index,
                                        row - plan.window.row_start,
                                        run.output + offset,
                                        0,
                                    ],
                                    vec![1, 1, len, channels],
                                )
                            } else {
                                (
                                    vec![
                                        output_index,
                                        row - plan.window.row_start,
                                        run.output + offset,
                                    ],
                                    vec![1, 1, len],
                                )
                            };
                            target.put_values(&values, (start, shape)).map_err(err)?;
                        }
                    }
                }
            }
            output_index += 1;
        }
    }
    Ok(plans.len())
}

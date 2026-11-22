//! Reusable bounded NetCDF geographic inspection, coverage, and export.
use super::{
    geographic_coordinate_role, select_geographic_axes, CoordinateCandidate, File, GeolocationRule,
    StagedOutput,
};
use netcdf::{self, AttributeValue as Attr, Variable};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub struct SubsetBounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}
impl SubsetBounds {
    pub fn valid(self) -> bool {
        [self.west, self.south, self.east, self.north]
            .iter()
            .all(|x| x.is_finite())
            && (-180.0..=180.0).contains(&self.west)
            && (-180.0..=180.0).contains(&self.east)
            && (-90.0..=90.0).contains(&self.south)
            && (-90.0..=90.0).contains(&self.north)
            && self.west != self.east
            && self.south < self.north
    }
    fn longitude(self, x: f64) -> f64 {
        let x = (x + 180.0).rem_euclid(360.0) - 180.0;
        if self.east == 180.0 && x == -180.0 {
            180.0
        } else {
            x
        }
    }
    pub fn contains(self, lat: f64, lon: f64) -> bool {
        if !lat.is_finite() || !lon.is_finite() {
            return false;
        }
        let lon = self.longitude(lon);
        (self.south..=self.north).contains(&lat)
            && if self.west < self.east {
                (self.west..=self.east).contains(&lon)
            } else {
                lon >= self.west || lon <= self.east
            }
    }
    fn width(self) -> f64 {
        if self.west == -180.0 && self.east == 180.0 {
            360.0
        } else {
            (self.east - self.west).rem_euclid(360.0)
        }
    }
    fn offset(self, lon: f64) -> f64 {
        (self.longitude(lon) - self.west).rem_euclid(360.0)
    }
}

#[derive(Clone, Debug)]
pub struct SubsetVariable {
    pub path: String,
    pub shape: Vec<usize>,
    pub latitude: String,
    pub longitude: String,
    pub regular_grid: bool,
    pub time: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubsetMode {
    Native,
    Mosaic,
}
#[derive(Clone, Debug)]
pub struct SubsetRequest {
    pub products: Vec<PathBuf>,
    pub variables: Vec<SubsetVariable>,
    pub bounds: SubsetBounds,
    pub from_date: String,
    pub through_date: String,
    pub mode: SubsetMode,
    pub resolution: f64,
    pub output: PathBuf,
}
#[derive(Clone, Debug)]
pub struct CoveragePreview {
    pub width: usize,
    pub height: usize,
    pub counts: Vec<u16>,
    pub products: usize,
    pub sampled_points: usize,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn numeric(v: &Variable<'_>) -> bool {
    matches!(
        v.vartype(),
        netcdf::types::NcVariableType::Float(_) | netcdf::types::NcVariableType::Int(_)
    )
}
fn attr_string(v: &Variable<'_>, name: &str) -> String {
    match v.attribute_value(name).and_then(Result::ok) {
        Some(Attr::Str(s)) => s,
        _ => String::new(),
    }
}
fn attr_number(v: &Variable<'_>, name: &str) -> Option<f64> {
    use Attr::*;
    match v.attribute_value(name).and_then(Result::ok)? {
        Double(x) => Some(x),
        Float(x) => Some(x as f64),
        Int(x) => Some(x as f64),
        Short(x) => Some(x as f64),
        Schar(x) => Some(x as f64),
        Uint(x) => Some(x as f64),
        Ushort(x) => Some(x as f64),
        Uchar(x) => Some(x as f64),
        Longlong(x) => Some(x as f64),
        Ulonglong(x) => Some(x as f64),
        _ => None,
    }
}
fn all_variables(file: &netcdf::File) -> Result<Vec<String>, String> {
    fn visit(group: &netcdf::Group<'_>, prefix: &str, paths: &mut Vec<String>) {
        for v in group.variables() {
            paths.push(format!("{prefix}{}", v.name()));
        }
        for sub in group.groups() {
            let path = format!("{prefix}{}/", sub.name());
            visit(&sub, &path, paths);
        }
    }
    let mut paths = file.variables().map(|v| v.name()).collect::<Vec<_>>();
    for group in file.groups().map_err(err)? {
        let prefix = format!("{}/", group.name());
        visit(&group, &prefix, &mut paths);
    }
    Ok(paths)
}
fn coordinate_role(path: &str, v: &Variable<'_>) -> Option<super::CoordinateRole> {
    geographic_coordinate_role(
        path,
        &attr_string(v, "standard_name"),
        &attr_string(v, "units"),
    )
}
/// Find numeric fields with regular latitude/longitude axes or native 2D swath coordinates.
pub fn inspect_subset_product(path: &Path) -> Result<Vec<SubsetVariable>, String> {
    inspect_subset_product_with_rules(path, &[])
}

/// Prefer H5Jive's configured geolocation discovery, then regular coordinate axes.
pub fn inspect_subset_product_with_rules(
    path: &Path,
    rules: &[GeolocationRule],
) -> Result<Vec<SubsetVariable>, String> {
    let file = netcdf::open(path).map_err(err)?;
    let reader = File::open(path).ok();
    let paths = all_variables(&file)?;
    let coords = paths
        .iter()
        .filter_map(|path| {
            let v = file.variable(path)?;
            numeric(&v).then(|| {
                coordinate_role(path, &v).map(|role| CoordinateCandidate {
                    path: path.clone(),
                    role,
                    shape: v.dimensions().iter().map(|d| d.len()).collect(),
                })
            })?
        })
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for path in &paths {
        let Some(v) = file.variable(path) else {
            continue;
        };
        if !numeric(&v) || coordinate_role(path, &v).is_some() {
            continue;
        }
        let shape = v.dimensions().iter().map(|d| d.len()).collect::<Vec<_>>();
        if !(2..=3).contains(&shape.len()) || shape.iter().any(|&x| x == 0) {
            continue;
        }
        let preferred = reader.as_ref().and_then(|reader| {
            let dataset_path = format!("/{}", path.trim_start_matches('/'));
            reader
                .geolocation_with_rules(&dataset_path, rules)
                .ok()
                .flatten()
        });
        let preferred_candidates = preferred.as_ref().and_then(|geo| {
            let latitude = geo.latitude.trim_start_matches('/');
            let longitude = geo.longitude.trim_start_matches('/');
            let lat = file.variable(latitude)?;
            let lon = file.variable(longitude)?;
            if !numeric(&lat) || !numeric(&lon) {
                return None;
            }
            Some((
                CoordinateCandidate {
                    path: latitude.to_owned(),
                    role: super::CoordinateRole::Latitude,
                    shape: lat.dimensions().iter().map(|d| d.len()).collect(),
                },
                CoordinateCandidate {
                    path: longitude.to_owned(),
                    role: super::CoordinateRole::Longitude,
                    shape: lon.dimensions().iter().map(|d| d.len()).collect(),
                },
            ))
        });
        let preferred = preferred_candidates.as_ref().map(|(lat, lon)| (lat, lon));
        if let Some(axes) = select_geographic_axes(path, &shape, preferred, &coords) {
            let time = if shape.len() == 3 {
                let dim = v.dimensions()[0].name();
                paths
                    .iter()
                    .find(|p| {
                        p.rsplit('/').next() == Some(dim.as_str())
                            && file.variable(p).is_some_and(|t| {
                                t.dimensions().len() == 1 && t.dimensions()[0].len() == shape[0]
                            })
                    })
                    .cloned()
            } else {
                None
            };
            candidates.push(SubsetVariable {
                path: path.clone(),
                shape,
                latitude: axes.latitude,
                longitude: axes.longitude,
                regular_grid: axes.regular_grid,
                time,
            });
        }
    }
    Ok(candidates)
}
fn value(v: &Variable<'_>, index: &[usize]) -> Result<f64, String> {
    let data = v
        .get_values::<f64, _>((index.to_vec(), vec![1; index.len()]))
        .map_err(err)?;
    data.into_iter()
        .next()
        .ok_or_else(|| "Empty coordinate read".into())
}
fn row_coords(
    file: &netcdf::File,
    choice: &SubsetVariable,
    row: usize,
    col: usize,
    len: usize,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let lat = file
        .variable(&choice.latitude)
        .ok_or("Latitude variable missing")?;
    let lon = file
        .variable(&choice.longitude)
        .ok_or("Longitude variable missing")?;
    if choice.regular_grid {
        let latitude = value(&lat, &[row])?;
        let longitudes = lon
            .get_values::<f64, _>((vec![col], vec![len]))
            .map_err(err)?;
        Ok((vec![latitude; len], longitudes))
    } else {
        let start = vec![row, col];
        let count = vec![1, len];
        Ok((
            lat.get_values::<f64, _>((start.clone(), count.clone()))
                .map_err(err)?,
            lon.get_values::<f64, _>((start, count)).map_err(err)?,
        ))
    }
}
fn data_row(
    v: &Variable<'_>,
    choice: &SubsetVariable,
    time: usize,
    row: usize,
    col: usize,
    len: usize,
) -> Result<Vec<f64>, String> {
    let (start, count) = if choice.shape.len() == 3 {
        (vec![time, row, col], vec![1, 1, len])
    } else {
        (vec![row, col], vec![1, len])
    };
    v.get_values::<f64, _>((start, count)).map_err(err)
}
fn same_choice(file: &netcdf::File, choice: &SubsetVariable) -> Result<SubsetVariable, String> {
    let v = file
        .variable(&choice.path)
        .ok_or_else(|| format!("{} is missing", choice.path))?;
    let shape = v.dimensions().iter().map(|d| d.len()).collect::<Vec<_>>();
    if shape.len() != choice.shape.len() || shape.iter().any(|&n| n == 0) {
        return Err(format!("{} has incompatible shape {shape:?}", choice.path));
    }
    let rows = shape[shape.len() - 2];
    let cols = shape[shape.len() - 1];
    let latitude = file
        .variable(&choice.latitude)
        .ok_or("Latitude coordinate missing")?;
    let longitude = file
        .variable(&choice.longitude)
        .ok_or("Longitude coordinate missing")?;
    let lat = latitude
        .dimensions()
        .iter()
        .map(|d| d.len())
        .collect::<Vec<_>>();
    let lon = longitude
        .dimensions()
        .iter()
        .map(|d| d.len())
        .collect::<Vec<_>>();
    if if choice.regular_grid {
        lat.as_slice() != [rows] || lon.as_slice() != [cols]
    } else {
        lat.as_slice() != [rows, cols] || lon.as_slice() != [rows, cols]
    } {
        return Err(format!(
            "Coordinates for {} have incompatible shape",
            choice.path
        ));
    }
    let mut adjusted = choice.clone();
    adjusted.shape = shape;
    Ok(adjusted)
}
/// Low cost combined map preview; each product contributes once per sampled map cell.
pub fn preview_subset_coverage(
    products: &[PathBuf],
    choice: &SubsetVariable,
    max_points: usize,
) -> Result<CoveragePreview, String> {
    let (width, height) = (360, 180);
    let mut counts = vec![0_u16; width * height];
    let mut sampled_points = 0;
    for product in products {
        let file = netcdf::open(product).map_err(err)?;
        let choice = same_choice(&file, choice)?;
        let rows = choice.shape[choice.shape.len() - 2];
        let cols = choice.shape[choice.shape.len() - 1];
        let per_product = (max_points / products.len().max(1)).max(1);
        let stride = ((rows.saturating_mul(cols) / per_product).max(1) as f64)
            .sqrt()
            .ceil() as usize;
        let mut seen = vec![false; width * height];
        for row in (0..rows).step_by(stride.max(1)) {
            for chunk in (0..cols).step_by(4096) {
                let len = (cols - chunk).min(4096);
                let (lats, lons) = row_coords(&file, &choice, row, chunk, len)?;
                for local in (0..len).step_by(stride.max(1)) {
                    let lat = lats[local];
                    let lon = lons[local];
                    if !lat.is_finite() || !lon.is_finite() || !(-90.0..=90.0).contains(&lat) {
                        continue;
                    }
                    let x = (((lon + 180.0).rem_euclid(360.0)).floor() as usize).min(width - 1);
                    let y = ((90.0 - lat).floor().clamp(0.0, 179.0)) as usize;
                    seen[y * width + x] = true;
                    sampled_points += 1;
                }
            }
        }
        for (count, hit) in counts.iter_mut().zip(seen) {
            if hit {
                *count = count.saturating_add(1);
            }
        }
    }
    Ok(CoveragePreview {
        width,
        height,
        counts,
        products: products.len(),
        sampled_points,
    })
}
fn days(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=month_days).contains(&day) {
        return None;
    }
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    Some(era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468)
}
fn parse_day(s: &str) -> Option<i64> {
    let digits = s.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
    if digits.len() < 8 {
        return None;
    }
    days(
        digits[0..4].parse().ok()?,
        digits[4..6].parse().ok()?,
        digits[6..8].parse().ok()?,
    )
}
fn parse_origin_seconds(origin: &str) -> Option<f64> {
    let date = parse_day(origin)?;
    let time = origin
        .get(10..)?
        .trim()
        .trim_start_matches('t')
        .trim_start_matches('T')
        .trim();
    if time.is_empty() {
        return Some(date as f64 * 86400.0);
    }
    let time = time
        .strip_suffix('z')
        .or_else(|| time.strip_suffix('Z'))
        .unwrap_or(time);
    if time.split_whitespace().count() != 1 {
        return None;
    }
    let parts = time.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let hour = parts[0].parse::<u32>().ok()?;
    let minute = parts[1].parse::<u32>().ok()?;
    let second = if parts.len() == 3 {
        parts[2].parse::<f64>().ok()?
    } else {
        0.0
    };
    if hour > 23 || minute > 59 || !(0.0..60.0).contains(&second) {
        return None;
    }
    Some(date as f64 * 86400.0 + hour as f64 * 3600.0 + minute as f64 * 60.0 + second)
}
fn time_window(
    file: &netcdf::File,
    choice: &SubsetVariable,
    from: &str,
    through: &str,
) -> Result<(usize, usize), String> {
    if choice.shape.len() == 2 {
        return Ok((0, 1));
    }
    let len = choice.shape[0];
    if from.trim().is_empty() && through.trim().is_empty() {
        return Ok((0, len));
    }
    let path = choice
        .time
        .as_ref()
        .ok_or("Time range requested, but no matching time coordinate was found")?;
    let v = file.variable(path).ok_or("Time coordinate missing")?;
    let calendar = attr_string(&v, "calendar").to_ascii_lowercase();
    if !calendar.is_empty()
        && !["standard", "gregorian", "proleptic_gregorian"].contains(&calendar.as_str())
    {
        return Err(format!("Unsupported CF calendar: {calendar}"));
    }
    let units = attr_string(&v, "units").to_ascii_lowercase();
    let (unit, origin) = units
        .split_once(" since ")
        .ok_or("Time coordinate needs CF units such as 'hours since YYYY-MM-DD'")?;
    let base = parse_origin_seconds(origin).ok_or("Could not parse CF time origin")?;
    let seconds = if unit.starts_with("day") {
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
    let from_day = if from.trim().is_empty() {
        i64::MIN
    } else {
        parse_day(from).ok_or("Invalid start date")?
    };
    let through_day = if through.trim().is_empty() {
        i64::MAX
    } else {
        parse_day(through).ok_or("Invalid end date")?
    };
    let mut first = None;
    let mut last = 0;
    for start in (0..len).step_by(4096) {
        let count = (len - start).min(4096);
        let times = v
            .get_values::<f64, _>((vec![start], vec![count]))
            .map_err(err)?;
        for (offset, time) in times.into_iter().enumerate() {
            let i = start + offset;
            let day = ((base + time * seconds) / 86400.0).floor() as i64;
            if day >= from_day && day <= through_day {
                first.get_or_insert(i);
                last = i + 1;
            }
        }
    }
    first
        .map(|i| (i, last))
        .ok_or_else(|| "No time samples in requested date range".into())
}
fn spatial_window(
    file: &netcdf::File,
    choice: &SubsetVariable,
    bounds: SubsetBounds,
) -> Result<Option<(usize, usize, usize, usize)>, String> {
    let rows = choice.shape[choice.shape.len() - 2];
    let cols = choice.shape[choice.shape.len() - 1];
    let mut r0 = rows;
    let mut r1 = 0;
    let mut c0 = cols;
    let mut c1 = 0;
    for row in 0..rows {
        for col in (0..cols).step_by(4096) {
            let len = (cols - col).min(4096);
            let (lats, lons) = row_coords(file, choice, row, col, len)?;
            for i in 0..len {
                if bounds.contains(lats[i], lons[i]) {
                    r0 = r0.min(row);
                    r1 = r1.max(row + 1);
                    c0 = c0.min(col + i);
                    c1 = c1.max(col + i + 1);
                }
            }
        }
    }
    Ok((r0 < r1 && c0 < c1).then_some((r0, r1, c0, c1)))
}
fn copy_attrs(
    source: &Variable<'_>,
    target: &mut netcdf::VariableMut<'_>,
    skip: &[&str],
) -> Result<(), String> {
    for attr in source.attributes() {
        if skip.contains(&attr.name()) {
            continue;
        }
        target
            .put_attribute(attr.name(), attr.value().map_err(err)?)
            .map_err(err)?;
    }
    Ok(())
}
fn write_native_product(
    out: &mut netcdf::FileMut,
    source: &netcdf::File,
    choices: &[SubsetVariable],
    bounds: SubsetBounds,
    from: &str,
    through: &str,
    product_index: usize,
) -> Result<bool, String> {
    let first = same_choice(source, &choices[0])?;
    let Some((r0, r1, c0, c1)) = spatial_window(source, &first, bounds)? else {
        return Ok(false);
    };
    let (t0, t1) = time_window(source, &first, from, through)?;
    let prefix = format!("product_{product_index:04}");
    out.add_group(&prefix).map_err(err)?;
    for (name, len) in [("row", r1 - r0), ("column", c1 - c0)] {
        out.add_dimension(&format!("{prefix}/{name}"), len)
            .map_err(err)?;
    }
    if first.shape.len() == 3 {
        out.add_dimension(&format!("{prefix}/time"), t1 - t0)
            .map_err(err)?;
    }
    let dims = if first.shape.len() == 3 {
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
        if first.regular_grid {
            let (start, end) = if name == "latitude" {
                (r0, r1)
            } else {
                (c0, c1)
            };
            for i in (start..end).step_by(4096) {
                let len = (end - i).min(4096);
                let values = src
                    .get_values::<f64, _>((vec![i], vec![len]))
                    .map_err(err)?;
                dst.put_values(&values, ([i - start], [len])).map_err(err)?;
            }
        } else {
            for r in r0..r1 {
                for c in (c0..c1).step_by(4096) {
                    let len = (c1 - c).min(4096);
                    let row = src
                        .get_values::<f64, _>((vec![r, c], vec![1, len]))
                        .map_err(err)?;
                    dst.put_values(&row, ([r - r0, c - c0], [1, len]))
                        .map_err(err)?;
                }
            }
        }
    }
    if let Some(path) = &first.time {
        if first.shape.len() == 3 {
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
            if first.shape.len() == 3 {
                "time latitude longitude"
            } else {
                "latitude longitude"
            },
        )
        .map_err(err)?;
        let fill = attr_number(&src, "_FillValue");
        let missing = attr_number(&src, "missing_value");
        for t in t0..t1 {
            for r in r0..r1 {
                for c in (c0..c1).step_by(4096) {
                    let len = (c1 - c).min(4096);
                    let (lats, lons) = row_coords(source, &choice, r, c, len)?;
                    let mut row = data_row(&src, &choice, t, r, c, len)?;
                    for i in 0..len {
                        if !bounds.contains(lats[i], lons[i])
                            || !row[i].is_finite()
                            || fill == Some(row[i])
                            || missing == Some(row[i])
                        {
                            row[i] = f64::NAN;
                        }
                    }
                    let (start, count) = if choice.shape.len() == 3 {
                        (vec![t - t0, r - r0, c - c0], vec![1, 1, len])
                    } else {
                        (vec![r - r0, c - c0], vec![1, len])
                    };
                    dst.put_values(&row, (start, count)).map_err(err)?;
                }
            }
        }
    }
    Ok(true)
}
fn write_mosaic(
    out: &mut netcdf::FileMut,
    products: &[PathBuf],
    choices: &[SubsetVariable],
    bounds: SubsetBounds,
    from: &str,
    through: &str,
    resolution: f64,
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
    out.add_dimension("latitude", height).map_err(err)?;
    out.add_dimension("longitude", width).map_err(err)?;
    let lat = (0..height)
        .map(|i| bounds.south + (i as f64 + 0.5) * resolution)
        .collect::<Vec<_>>();
    let lon = (0..width)
        .map(|i| {
            let x = bounds.west + (i as f64 + 0.5) * resolution;
            (x + 180.0).rem_euclid(360.0) - 180.0
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
    let mut used = std::collections::HashSet::new();
    for choice in choices {
        let mut sums = vec![0.0; cells];
        let mut counts = vec![0_u32; cells];
        for path in products {
            let source = netcdf::open(path).map_err(err)?;
            let choice = same_choice(&source, choice)?;
            let (t0, t1) = time_window(&source, &choice, from, through)?;
            let src = source.variable(&choice.path).ok_or("Variable missing")?;
            let scale = attr_number(&src, "scale_factor").unwrap_or(1.0);
            let offset = attr_number(&src, "add_offset").unwrap_or(0.0);
            let rows = choice.shape[choice.shape.len() - 2];
            let cols = choice.shape[choice.shape.len() - 1];
            let mut hit = false;
            let fill = attr_number(&src, "_FillValue");
            let missing = attr_number(&src, "missing_value");
            for r in 0..rows {
                for c in (0..cols).step_by(4096) {
                    let len = (cols - c).min(4096);
                    let (lats, lons) = row_coords(&source, &choice, r, c, len)?;
                    for t in t0..t1 {
                        let values = data_row(&src, &choice, t, r, c, len)?;
                        for i in 0..len {
                            let la = lats[i];
                            let lo = lons[i];
                            let raw = values[i];
                            if !bounds.contains(la, lo)
                                || !raw.is_finite()
                                || fill == Some(raw)
                                || missing == Some(raw)
                            {
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
                            let v = raw * scale + offset;
                            if v.is_finite() {
                                let cell = y * width + x;
                                sums[cell] += v;
                                counts[cell] = counts[cell].saturating_add(1);
                                hit = true;
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
        let means = sums
            .into_iter()
            .zip(&counts)
            .map(|(sum, &n)| if n > 0 { sum / n as f64 } else { f64::NAN })
            .collect::<Vec<_>>();
        let mut dst = out
            .add_variable::<f64>(name, &["latitude", "longitude"])
            .map_err(err)?;
        dst.set_fill_value(f64::NAN).map_err(err)?;
        dst.put_values(&means, ..).map_err(err)?;
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
        dst.put_attribute("coordinates", "latitude longitude")
            .map_err(err)?;
        drop(dst);
        out.add_variable::<u32>(&format!("{name}_count"), &["latitude", "longitude"])
            .map_err(err)?
            .put_values(&counts, ..)
            .map_err(err)?;
    }
    Ok(used.len())
}
/// Write a new NetCDF-4 file using a temporary sibling, leaving existing files untouched.
pub fn export_subset(request: &SubsetRequest) -> Result<String, String> {
    if !request.bounds.valid() {
        return Err("Invalid geographic bounds".into());
    }
    if request.products.is_empty() || request.variables.is_empty() {
        return Err("Select products and variables".into());
    }
    if request.output.exists() {
        return Err("Output file already exists".into());
    }
    let staged_output = StagedOutput::new(&request.output, "subset", "nc").map_err(err)?;
    let result = (|| {
        let mut out = netcdf::create(staged_output.path()).map_err(err)?;
        out.add_attribute("Conventions", "CF-1.10").map_err(err)?;
        out.add_attribute("history", "Subset exported by H5Jive")
            .map_err(err)?;
        out.add_attribute("geospatial_lat_min", request.bounds.south)
            .map_err(err)?;
        out.add_attribute("geospatial_lat_max", request.bounds.north)
            .map_err(err)?;
        out.add_attribute("geospatial_lon_min", request.bounds.west)
            .map_err(err)?;
        out.add_attribute("geospatial_lon_max", request.bounds.east)
            .map_err(err)?;
        let mut used = 0;
        match request.mode {
            SubsetMode::Native => {
                for (i, path) in request.products.iter().enumerate() {
                    let source = netcdf::open(path).map_err(err)?;
                    for attr in source.attributes() {
                        let key = format!("source_{i:04}_{}", attr.name());
                        if let Ok(value) = attr.value() {
                            let _ = out.add_attribute(&key, value);
                        }
                    }
                    if write_native_product(
                        &mut out,
                        &source,
                        &request.variables,
                        request.bounds,
                        &request.from_date,
                        &request.through_date,
                        i,
                    )? {
                        out.add_attribute(
                            &format!("product_{i:04}/source_file"),
                            path.to_string_lossy().as_ref(),
                        )
                        .map_err(err)?;
                        used += 1;
                    }
                }
            }
            SubsetMode::Mosaic => {
                used = write_mosaic(
                    &mut out,
                    &request.products,
                    &request.variables,
                    request.bounds,
                    &request.from_date,
                    &request.through_date,
                    request.resolution,
                )?
            }
        }
        if used == 0 {
            return Err("No selected product has values inside the bounds and time range".into());
        }
        out.close().map_err(err)?;
        staged_output.publish_new(&request.output).map_err(err)?;
        Ok(format!(
            "Exported {used} products to {}",
            request.output.display()
        ))
    })();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    static NEXT_DIRECTORY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn directory() -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_DIRECTORY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "h5jive-subset-test-{}-{stamp}-{id}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }
    fn swath(path: &Path, rows: usize, start_lat: f64, value: f64) {
        let mut file = netcdf::create(path).unwrap();
        file.add_dimension("row", rows).unwrap();
        file.add_dimension("column", 2).unwrap();
        let lats = (0..rows)
            .flat_map(|r| [start_lat + r as f64; 2])
            .collect::<Vec<_>>();
        let lons = (0..rows).flat_map(|_| [0.0, 1.0]).collect::<Vec<_>>();
        file.add_variable::<f64>("latitude", &["row", "column"])
            .unwrap()
            .put_values(&lats, ..)
            .unwrap();
        file.add_variable::<f64>("longitude", &["row", "column"])
            .unwrap()
            .put_values(&lons, ..)
            .unwrap();
        file.variable_mut("latitude")
            .unwrap()
            .put_attribute("units", "degrees_north")
            .unwrap();
        file.variable_mut("longitude")
            .unwrap()
            .put_attribute("units", "degrees_east")
            .unwrap();
        let mut u = file.add_variable::<f32>("u10", &["row", "column"]).unwrap();
        u.put_attribute("units", "m s-1").unwrap();
        u.put_values(&vec![value as f32; rows * 2], ..).unwrap();
        file.close().unwrap();
    }
    fn request(paths: Vec<PathBuf>, output: PathBuf, mode: SubsetMode) -> SubsetRequest {
        let choices = inspect_subset_product(&paths[0]).unwrap();
        SubsetRequest {
            products: paths,
            variables: vec![choices.into_iter().find(|v| v.path == "u10").unwrap()],
            bounds: SubsetBounds {
                west: -0.5,
                south: 50.0,
                east: 1.5,
                north: 53.0,
            },
            from_date: String::new(),
            through_date: String::new(),
            mode,
            resolution: 1.0,
            output,
        }
    }
    #[test]
    fn native_keeps_different_swath_shapes_and_coordinates() {
        let dir = directory();
        let a = dir.join("a.nc");
        let b = dir.join("b.nc");
        let output = dir.join("native.nc");
        swath(&a, 2, 50.0, 2.0);
        swath(&b, 3, 51.0, 6.0);
        let request = request(vec![a, b], output.clone(), SubsetMode::Native);
        let preview =
            preview_subset_coverage(&request.products, &request.variables[0], 1000).unwrap();
        assert_eq!(preview.products, 2);
        assert!(preview.counts.iter().any(|&n| n == 2));
        export_subset(&request).unwrap();
        let file = netcdf::open(&output).unwrap();
        let first = file.variable("product_0000/u10").unwrap();
        let second = file.variable("product_0001/u10").unwrap();
        assert_eq!(
            first
                .dimensions()
                .iter()
                .map(|d| d.len())
                .collect::<Vec<_>>(),
            vec![2, 2]
        );
        assert_eq!(
            second
                .dimensions()
                .iter()
                .map(|d| d.len())
                .collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert_eq!(second.get_values::<f64, _>(..).unwrap(), vec![6.0; 6]);
        assert_eq!(
            first.attribute_value("units").unwrap().unwrap(),
            Attr::Str("m s-1".into())
        );
        assert!(file
            .group("product_0001")
            .unwrap()
            .unwrap()
            .attribute("source_file")
            .is_some());
        assert_eq!(
            file.variable("product_0001/latitude")
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap(),
            vec![51.0, 51.0, 52.0, 52.0, 53.0, 53.0]
        );
        assert!(export_subset(&request).is_err());
    }
    #[test]
    fn mosaic_averages_overlap_and_counts_samples() {
        let dir = directory();
        let a = dir.join("a.nc");
        let b = dir.join("b.nc");
        let output = dir.join("mosaic.nc");
        swath(&a, 2, 50.0, 2.0);
        swath(&b, 2, 51.0, 6.0);
        let request = request(vec![a, b], output.clone(), SubsetMode::Mosaic);
        export_subset(&request).unwrap();
        let file = netcdf::open(output).unwrap();
        let means = file
            .variable("u10")
            .unwrap()
            .get_values::<f64, _>(..)
            .unwrap();
        let counts = file
            .variable("u10_count")
            .unwrap()
            .get_values::<u32, _>(..)
            .unwrap();
        assert!(means.iter().any(|&x| x == 4.0));
        assert!(counts.iter().any(|&x| x == 2));
    }
    #[test]
    fn regular_grid_crops_cf_time() {
        let dir = directory();
        let input = dir.join("grid.nc");
        let output = dir.join("one_week.nc");
        let mut file = netcdf::create(&input).unwrap();
        for (name, len) in [("time", 4), ("lat", 2), ("lon", 2)] {
            file.add_dimension(name, len).unwrap();
        }
        file.add_variable::<f64>("lat", &["lat"])
            .unwrap()
            .put_values(&[50.0, 51.0], ..)
            .unwrap();
        file.add_variable::<f64>("lon", &["lon"])
            .unwrap()
            .put_values(&[0.0, 1.0], ..)
            .unwrap();
        let mut t = file.add_variable::<f64>("time", &["time"]).unwrap();
        t.put_attribute("units", "hours since 2024-01-01 00:00:00")
            .unwrap();
        t.put_values(&[0.0, 24.0, 48.0, 72.0], ..).unwrap();
        file.add_variable::<f32>("u10", &["time", "lat", "lon"])
            .unwrap()
            .put_values(&(0..16).map(|x| x as f32).collect::<Vec<_>>(), ..)
            .unwrap();
        file.close().unwrap();
        let mut request = request(vec![input], output.clone(), SubsetMode::Native);
        request.from_date = "2024-01-02".into();
        request.through_date = "2024-01-03".into();
        export_subset(&request).unwrap();
        let out = netcdf::open(output).unwrap();
        let u = out.variable("product_0000/u10").unwrap();
        assert_eq!(
            u.dimensions().iter().map(|d| d.len()).collect::<Vec<_>>(),
            vec![2, 2, 2]
        );
        assert_eq!(
            u.get_values::<f64, _>(..).unwrap(),
            (4..12).map(|x| x as f64).collect::<Vec<_>>()
        );
    }
    #[test]
    fn bounds_include_dateline_and_full_world() {
        let b = SubsetBounds {
            west: 170.0,
            south: -5.0,
            east: -170.0,
            north: 5.0,
        };
        assert!(b.contains(0.0, 179.0));
        assert!(b.contains(0.0, -179.0));
        assert!(!b.contains(0.0, 0.0));
        let world = SubsetBounds {
            west: -180.0,
            south: -90.0,
            east: 180.0,
            north: 90.0,
        };
        assert_eq!(world.width(), 360.0);
        assert!(world.contains(0.0, 0.0));
        assert_eq!(
            parse_origin_seconds("2024-01-01 12:00:00"),
            parse_origin_seconds("2024-01-01").map(|s| s + 43200.0)
        );
        assert!(parse_origin_seconds("2024-02-31 00:00:00").is_none());
    }
}

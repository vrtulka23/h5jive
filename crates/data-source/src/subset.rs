//! Reusable bounded NetCDF geographic inspection, coverage, and export.
use super::{
    geographic_coordinate_role, select_geographic_axes, CoordinateCandidate, File, Geolocation,
    GeolocationRule, StagedOutput,
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
fn spatial_shape(choice: &SubsetVariable) -> (usize, usize) {
    let axes = if choice.shape.len() == 4 {
        &choice.shape[1..3]
    } else {
        &choice.shape[choice.shape.len() - 2..]
    };
    (axes[0], axes[1])
}
fn coordinate_layout(shape: &[usize], latitude: &[usize], longitude: &[usize]) -> Option<bool> {
    let (rows, cols) = if shape.len() == 4 {
        (shape[1], shape[2])
    } else {
        (shape[shape.len() - 2], shape[shape.len() - 1])
    };
    if latitude == [rows] && longitude == [cols] {
        Some(true)
    } else if latitude == [rows, cols] && longitude == [rows, cols] {
        Some(false)
    } else if shape.len() >= 3
        && (latitude == [shape[0], rows, cols] || latitude == [1, rows, cols])
        && (longitude == [shape[0], rows, cols] || longitude == [1, rows, cols])
    {
        Some(false)
    } else {
        None
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubsetMode {
    Native,
    TimeSeries,
    Mosaic,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MosaicMethod {
    Mean,
    Newest,
    Oldest,
    Minimum,
    Maximum,
}
impl MosaicMethod {
    fn label(self) -> &'static str {
        match self {
            Self::Mean => "mean",
            Self::Newest => "newest",
            Self::Oldest => "oldest",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
        }
    }
}
#[derive(Clone, Debug)]
pub struct SubsetRequest {
    pub products: Vec<PathBuf>,
    pub variables: Vec<SubsetVariable>,
    pub bounds: SubsetBounds,
    pub from_date: String,
    pub through_date: String,
    pub mode: SubsetMode,
    /// Optional NetCDF group for a combined time series; empty writes at the root.
    pub series_group: String,
    /// Leading dimension name when the source has no CF time coordinate.
    pub series_dimension: String,
    pub mosaic_method: MosaicMethod,
    pub resolution: f64,
    pub output: PathBuf,
    pub geolocation_rules: Vec<GeolocationRule>,
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
    let root_attributes = reader
        .as_ref()
        .and_then(|reader| reader.attributes("/").ok())
        .unwrap_or_default();
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
        if !(2..=4).contains(&shape.len()) || shape.iter().any(|&x| x == 0) {
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
        let axes = preferred_candidates
            .as_ref()
            .and_then(|(lat, lon)| {
                coordinate_layout(&shape, &lat.shape, &lon.shape).map(|regular_grid| {
                    super::GeographicAxes {
                        latitude: lat.path.clone(),
                        longitude: lon.path.clone(),
                        regular_grid,
                    }
                })
            })
            .or_else(|| {
                if shape.len() == 4 {
                    select_geographic_axes(path, &shape[1..3], preferred, &coords)
                } else {
                    select_geographic_axes(path, &shape, preferred, &coords)
                }
            });
        if let Some(axes) = axes {
            let time = if shape.len() >= 3 {
                let dim = v.dimensions()[0].name();
                let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
                let group = parent.rsplit('/').next().unwrap_or("");
                let configured = reader.as_ref().and_then(|reader| {
                    rules
                        .iter()
                        .find(|rule| {
                            rule.observation_group == group
                                && rule.matches(&reader.name, &root_attributes)
                        })
                        .map(|rule| rule.time_coordinate.trim())
                        .filter(|name| !name.is_empty())
                });
                let find_time = |name: &str| {
                    paths
                        .iter()
                        .filter(|candidate| candidate.rsplit('/').next() == Some(name))
                        .filter(|candidate| {
                            file.variable(candidate).is_some_and(|time| {
                                numeric(&time)
                                    && time.dimensions().len() == 1
                                    && time.dimensions()[0].name() == dim
                            })
                        })
                        .min_by_key(|candidate| {
                            candidate.rsplit_once('/').map_or("", |(scope, _)| scope) != parent
                        })
                        .cloned()
                };
                configured
                    .and_then(|name| find_time(name))
                    .or_else(|| find_time(&dim))
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
        let coordinate_rank = lat.dimensions().len();
        let (start, count) = if coordinate_rank == 3 {
            (vec![0, row, col], vec![1, 1, len])
        } else {
            (vec![row, col], vec![1, len])
        };
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
    let (start, count) = if choice.shape.len() == 4 {
        (vec![time, row, col, 0], vec![1, 1, len, choice.shape[3]])
    } else if choice.shape.len() == 3 {
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
    if coordinate_layout(&shape, &lat, &lon) != Some(choice.regular_grid) {
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
        let (rows, cols) = spatial_shape(&choice);
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
    if len == 1 && choice.time.is_none() {
        return Ok((0, 1));
    }
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
struct ColumnRun {
    source: usize,
    output: usize,
    count: usize,
}
struct SpatialWindow {
    row_start: usize,
    row_end: usize,
    columns: Vec<ColumnRun>,
    width: usize,
}
fn spatial_window(
    file: &netcdf::File,
    choice: &SubsetVariable,
    bounds: SubsetBounds,
) -> Result<Option<SpatialWindow>, String> {
    let (rows, cols) = spatial_shape(choice);
    let mut r0 = rows;
    let mut r1 = 0;
    let mut c0 = cols;
    let mut c1 = 0;
    let mut selected_columns = vec![false; cols];
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
                    selected_columns[col + i] = true;
                }
            }
        }
    }
    if r0 >= r1 || c0 >= c1 {
        return Ok(None);
    }
    let ordered = if choice.regular_grid {
        let longitude = file
            .variable(&choice.longitude)
            .ok_or("Longitude variable missing")?
            .get_values::<f64, _>(..)
            .map_err(err)?;
        let mut columns = (0..cols)
            .filter(|&index| selected_columns[index])
            .collect::<Vec<_>>();
        columns.sort_by(|&left, &right| {
            bounds
                .offset(longitude[left])
                .total_cmp(&bounds.offset(longitude[right]))
        });
        columns
    } else {
        (c0..c1).collect()
    };
    let width = ordered.len();
    let mut runs: Vec<ColumnRun> = Vec::new();
    for (output, source) in ordered.into_iter().enumerate() {
        if let Some(last) = runs.last_mut() {
            if last.source + last.count == source && last.output + last.count == output {
                last.count += 1;
                continue;
            }
        }
        runs.push(ColumnRun {
            source,
            output,
            count: 1,
        });
    }
    Ok(Some(SpatialWindow {
        row_start: r0,
        row_end: r1,
        columns: runs,
        width,
    }))
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
        for (name, path) in [
            ("latitude_bounds", geo.latitude_bounds.as_ref()),
            ("longitude_bounds", geo.longitude_bounds.as_ref()),
            (
                "scan_direction",
                geo.scan_direction.as_ref().map(|scan| &scan.path),
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
fn netcdf_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

struct SeriesProduct {
    path: PathBuf,
    source_index: usize,
    window: SpatialWindow,
    times: Vec<(usize, Option<f64>)>,
    geolocation: Option<Geolocation>,
}

fn series_row_coords(
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

fn series_aux_row(
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

fn series_axis(
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

fn series_time_values(
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

fn write_time_series(out: &mut netcdf::FileMut, request: &SubsetRequest) -> Result<usize, String> {
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
                                    let values = series_aux_row(
                                        &file,
                                        path,
                                        source_time,
                                        row,
                                        column,
                                        len,
                                        bounds,
                                    )?;
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

fn write_mosaic(
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
                        path,
                        &request.variables,
                        &request.geolocation_rules,
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
            SubsetMode::TimeSeries => {
                used = write_time_series(&mut out, request)?;
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
                    request.mosaic_method,
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
    fn series_grid(path: &Path, times: Option<&[f64]>, value: f32) {
        let mut file = netcdf::create(path).unwrap();
        file.add_dimension("lat", 2).unwrap();
        file.add_dimension("lon", 2).unwrap();
        file.add_variable::<f64>("lat", &["lat"])
            .unwrap()
            .put_values(&[50.0, 51.0], ..)
            .unwrap();
        file.add_variable::<f64>("lon", &["lon"])
            .unwrap()
            .put_values(&[0.0, 1.0], ..)
            .unwrap();
        if let Some(times) = times {
            file.add_dimension("time", times.len()).unwrap();
            let mut time = file.add_variable::<f64>("time", &["time"]).unwrap();
            time.put_attribute("units", "hours since 2024-01-01 00:00:00")
                .unwrap();
            time.put_values(times, ..).unwrap();
            file.add_variable::<f32>("u10", &["time", "lat", "lon"])
                .unwrap()
                .put_values(&vec![value; times.len() * 4], ..)
                .unwrap();
        } else {
            file.add_variable::<f32>("u10", &["lat", "lon"])
                .unwrap()
                .put_values(&vec![value; 4], ..)
                .unwrap();
        }
        file.close().unwrap();
    }

    #[test]
    fn configured_time_coordinate_can_differ_from_dimension_name() {
        let dir = directory();
        let input = dir.join("custom_time.nc");
        let output = dir.join("custom_time_series.nc");
        let mut file = netcdf::create(&input).unwrap();
        for (name, values) in [("lat", vec![50.0, 51.0]), ("lon", vec![0.0, 1.0])] {
            file.add_dimension(name, 2).unwrap();
            file.add_variable::<f64>(name, &[name])
                .unwrap()
                .put_values(&values, ..)
                .unwrap();
        }
        file.add_dimension("sample", 2).unwrap();
        let mut time = file
            .add_variable::<f64>("observation_time", &["sample"])
            .unwrap();
        time.put_attribute("units", "hours since 2024-01-01 00:00:00")
            .unwrap();
        time.put_values(&[0.0, 24.0], ..).unwrap();
        file.add_variable::<f32>("u10", &["sample", "lat", "lon"])
            .unwrap()
            .put_values(&vec![1.0; 8], ..)
            .unwrap();
        file.close().unwrap();

        assert!(inspect_subset_product(&input)
            .unwrap()
            .into_iter()
            .find(|choice| choice.path == "u10")
            .unwrap()
            .time
            .is_none());
        let rule = GeolocationRule {
            file_glob: "custom_time.nc".into(),
            platform: String::new(),
            sensor: String::new(),
            product_type: String::new(),
            observation_group: String::new(),
            coordinate_group: String::new(),
            latitude: "lat".into(),
            longitude: "lon".into(),
            latitude_bounds: String::new(),
            longitude_bounds: String::new(),
            time_coordinate: "observation_time".into(),
            scan_direction: None,
        };
        let choice = inspect_subset_product_with_rules(&input, &[rule.clone()])
            .unwrap()
            .into_iter()
            .find(|choice| choice.path == "u10")
            .unwrap();
        assert_eq!(choice.time.as_deref(), Some("observation_time"));
        let mut request = request(vec![input], output.clone(), SubsetMode::TimeSeries);
        request.variables = vec![choice];
        request.geolocation_rules = vec![rule];
        export_subset(&request).unwrap();
        let file = netcdf::open(output).unwrap();
        assert_eq!(
            file.variable("u10").unwrap().dimensions()[0].name(),
            "sample"
        );
        assert_eq!(file.variable("sample").unwrap().len(), 2);
    }

    #[test]
    fn combined_time_series_sorts_products_and_reuses_cf_time() {
        let dir = directory();
        let later = dir.join("later.nc");
        let earlier = dir.join("earlier.nc");
        let output = dir.join("series.nc");
        series_grid(&later, Some(&[48.0, 72.0]), 2.0);
        series_grid(&earlier, Some(&[0.0, 1.0]), 1.0);
        let mut earlier_file = netcdf::append(&earlier).unwrap();
        earlier_file
            .variable_mut("time")
            .unwrap()
            .put_attribute("units", "days since 2024-01-01 00:00:00")
            .unwrap();
        earlier_file.close().unwrap();
        let mut request = request(
            vec![later.clone(), earlier.clone()],
            output.clone(),
            SubsetMode::TimeSeries,
        );
        request.series_group = "winds".into();
        export_subset(&request).unwrap();
        let file = netcdf::open(output).unwrap();
        let time = file.variable("winds/time").unwrap();
        let values = time.get_values::<f64, _>(..).unwrap();
        assert_eq!(values.len(), 4);
        assert_eq!(values[1] - values[0], 86400.0);
        assert_eq!(values[3] - values[2], 86400.0);
        assert_eq!(
            file.variable("winds/u10")
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap(),
            [vec![1.0; 8], vec![2.0; 8]].concat()
        );
        assert_eq!(
            file.variable("winds/source_product")
                .unwrap()
                .get_values::<i32, _>(..)
                .unwrap(),
            vec![1, 1, 0, 0]
        );
        drop(file);
        request.products = vec![later.clone(), later.clone()];
        request.output = dir.join("overlap.nc");
        assert!(export_subset(&request).unwrap_err().contains("overlap"));
        assert!(!request.output.exists());
        let mut earlier_file = netcdf::append(&earlier).unwrap();
        earlier_file
            .variable_mut("u10")
            .unwrap()
            .put_attribute("units", "K")
            .unwrap();
        earlier_file.close().unwrap();
        request.products = vec![later, earlier];
        request.output = dir.join("units.nc");
        assert!(export_subset(&request)
            .unwrap_err()
            .contains("different units"));
        assert!(!request.output.exists());
    }

    #[test]
    fn combined_series_without_time_uses_named_index_and_rejects_mismatched_grid() {
        let dir = directory();
        let a = dir.join("a.nc");
        let b = dir.join("b.nc");
        let output = dir.join("indexed.nc");
        series_grid(&a, None, 1.0);
        series_grid(&b, None, 2.0);
        let mut request = request(vec![a, b.clone()], output.clone(), SubsetMode::TimeSeries);
        request.series_dimension = "scene".into();
        export_subset(&request).unwrap();
        let file = netcdf::open(output).unwrap();
        assert_eq!(
            file.variable("u10")
                .unwrap()
                .dimensions()
                .iter()
                .map(|d| d.name())
                .collect::<Vec<_>>(),
            vec!["scene", "row", "column"]
        );
        assert_eq!(
            file.variable("scene")
                .unwrap()
                .get_values::<i64, _>(..)
                .unwrap(),
            vec![0, 1]
        );
        drop(file);
        let mut changed = netcdf::append(&b).unwrap();
        changed
            .variable_mut("lon")
            .unwrap()
            .put_value(2.0, [1])
            .unwrap();
        changed.close().unwrap();
        request.output = dir.join("invalid.nc");
        assert!(export_subset(&request)
            .unwrap_err()
            .contains("different cropped latitude/longitude grid"));
        assert!(!request.output.exists());
    }

    #[test]
    fn combined_era5_series_keeps_source_time_dimension_name() {
        let input = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/spatiotemporal_subset/era5/ERA5_2026_04.nc");
        let output = directory().join("era5_series.nc");
        let mut request = request(vec![input], output.clone(), SubsetMode::TimeSeries);
        request.bounds = SubsetBounds {
            west: 114.0,
            south: 21.0,
            east: 116.0,
            north: 24.0,
        };
        request.from_date = "2026-04-02".into();
        request.through_date = "2026-04-02".into();
        export_subset(&request).unwrap();
        let file = netcdf::open(output).unwrap();
        assert_eq!(
            file.variable("u10").unwrap().dimensions()[0].name(),
            "valid_time"
        );
        assert_eq!(file.variable("valid_time").unwrap().len(), 24);
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
            mosaic_method: MosaicMethod::Mean,
            series_group: String::new(),
            series_dimension: "product".into(),
            resolution: 1.0,
            output,
            geolocation_rules: Vec::new(),
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
    fn mosaic_methods_choose_expected_overlapping_values() {
        let dir = directory();
        let old = dir.join("old.nc");
        let new = dir.join("new.nc");
        swath(&old, 2, 50.0, 2.0);
        swath(&new, 2, 50.0, 6.0);
        for (method, expected) in [
            (MosaicMethod::Mean, 4.0),
            (MosaicMethod::Newest, 6.0),
            (MosaicMethod::Oldest, 2.0),
            (MosaicMethod::Minimum, 2.0),
            (MosaicMethod::Maximum, 6.0),
        ] {
            let output = dir.join(format!("{}.nc", method.label()));
            let mut request = request(
                vec![old.clone(), new.clone()],
                output.clone(),
                SubsetMode::Mosaic,
            );
            request.mosaic_method = method;
            export_subset(&request).unwrap();
            let file = netcdf::open(output).unwrap();
            let values = file
                .variable("u10")
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap();
            let counts = file
                .variable("u10_count")
                .unwrap()
                .get_values::<u32, _>(..)
                .unwrap();
            assert_eq!(values[0], expected, "{method:?}");
            assert_eq!(counts[0], 2, "{method:?}");
            assert_eq!(
                file.attribute("mosaic_method").unwrap().value().unwrap(),
                Attr::Str(method.label().into())
            );
        }
    }
    #[test]
    fn temporal_mosaic_uses_cf_time_values_instead_of_array_order() {
        let dir = directory();
        let input = dir.join("unsorted_time.nc");
        let mut file = netcdf::create(&input).unwrap();
        for (name, len) in [("time", 2), ("latitude", 2), ("longitude", 2)] {
            file.add_dimension(name, len).unwrap();
        }
        file.add_variable::<f64>("latitude", &["latitude"])
            .unwrap()
            .put_values(&[50.0, 51.0], ..)
            .unwrap();
        file.variable_mut("latitude")
            .unwrap()
            .put_attribute("units", "degrees_north")
            .unwrap();
        file.add_variable::<f64>("longitude", &["longitude"])
            .unwrap()
            .put_values(&[0.0, 1.0], ..)
            .unwrap();
        file.variable_mut("longitude")
            .unwrap()
            .put_attribute("units", "degrees_east")
            .unwrap();
        let mut time = file.add_variable::<f64>("time", &["time"]).unwrap();
        time.put_attribute("units", "hours since 2024-01-01 00:00:00")
            .unwrap();
        time.put_values(&[24.0, 0.0], ..).unwrap();
        file.add_variable::<f32>("u10", &["time", "latitude", "longitude"])
            .unwrap()
            .put_values(
                &[6.0; 4].into_iter().chain([2.0; 4]).collect::<Vec<_>>(),
                ..,
            )
            .unwrap();
        file.close().unwrap();
        for (method, expected) in [(MosaicMethod::Newest, 6.0), (MosaicMethod::Oldest, 2.0)] {
            let output = dir.join(format!("{}.nc", method.label()));
            let mut request = request(vec![input.clone()], output.clone(), SubsetMode::Mosaic);
            request.mosaic_method = method;
            export_subset(&request).unwrap();
            let file = netcdf::open(output).unwrap();
            assert_eq!(
                file.variable("u10")
                    .unwrap()
                    .get_value::<f64, _>([0, 0])
                    .unwrap(),
                expected
            );
            assert_eq!(
                file.variable("u10_count")
                    .unwrap()
                    .get_value::<u32, _>([0, 0])
                    .unwrap(),
                2
            );
        }
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
    fn regular_grid_wraps_longitude_without_exporting_empty_columns() {
        let dir = directory();
        let input = dir.join("wrapped.nc");
        let output = dir.join("north_sea.nc");
        let mut file = netcdf::create(&input).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_dimension("longitude", 10).unwrap();
        file.add_variable::<f64>("latitude", &["latitude"])
            .unwrap()
            .put_values(&[55.0, 56.0], ..)
            .unwrap();
        file.variable_mut("latitude")
            .unwrap()
            .put_attribute("units", "degrees_north")
            .unwrap();
        file.add_variable::<f64>("longitude", &["longitude"])
            .unwrap()
            .put_values(
                &[0.0, 1.0, 2.0, 3.0, 4.0, 355.0, 356.0, 357.0, 358.0, 359.0],
                ..,
            )
            .unwrap();
        file.variable_mut("longitude")
            .unwrap()
            .put_attribute("units", "degrees_east")
            .unwrap();
        file.add_variable::<f32>("u10", &["latitude", "longitude"])
            .unwrap()
            .put_values(&(0..20).map(|i| i as f32).collect::<Vec<_>>(), ..)
            .unwrap();
        file.close().unwrap();
        let variable = inspect_subset_product(&input)
            .unwrap()
            .into_iter()
            .find(|choice| choice.path == "u10")
            .unwrap();
        export_subset(&SubsetRequest {
            products: vec![input],
            variables: vec![variable],
            bounds: SubsetBounds {
                west: -2.0,
                south: 54.0,
                east: 2.0,
                north: 57.0,
            },
            from_date: String::new(),
            through_date: String::new(),
            mode: SubsetMode::Native,
            mosaic_method: MosaicMethod::Mean,
            series_group: String::new(),
            series_dimension: "product".into(),
            resolution: 1.0,
            output: output.clone(),
            geolocation_rules: Vec::new(),
        })
        .unwrap();
        let result = netcdf::open(output).unwrap();
        assert_eq!(
            result
                .variable("product_0000/longitude")
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap(),
            [-2.0, -1.0, 0.0, 1.0, 2.0]
        );
        assert_eq!(
            result
                .variable("product_0000/u10")
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap(),
            [8.0, 9.0, 0.0, 1.0, 2.0, 18.0, 19.0, 10.0, 11.0, 12.0]
        );
    }
    #[test]
    fn era5_example_crops_week_and_wind_components() {
        let input = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/spatiotemporal_subset/era5/ERA5_2026_04.nc");
        let output_dir = directory();
        let output = output_dir.join("wind_week.nc");
        let choices = inspect_subset_product(&input).unwrap();
        let variables = ["u10", "v10"]
            .iter()
            .map(|name| {
                choices
                    .iter()
                    .find(|choice| choice.path == *name)
                    .unwrap()
                    .clone()
            })
            .collect();
        export_subset(&SubsetRequest {
            products: vec![input.clone()],
            variables,
            bounds: SubsetBounds {
                west: 114.0,
                south: 21.0,
                east: 116.0,
                north: 24.0,
            },
            from_date: "2026-04-02".into(),
            through_date: "2026-04-08".into(),
            mode: SubsetMode::Native,
            mosaic_method: MosaicMethod::Mean,
            series_group: String::new(),
            series_dimension: "product".into(),
            resolution: 0.25,
            output: output.clone(),
            geolocation_rules: Vec::new(),
        })
        .unwrap();
        let original = netcdf::open(input).unwrap();
        let subset = netcdf::open(&output).unwrap();
        for (source_name, output_name, start, count) in [
            ("valid_time", "time", vec![24], vec![168]),
            ("latitude", "latitude", vec![4], vec![13]),
            ("longitude", "longitude", vec![4], vec![9]),
        ] {
            let source = original
                .variable(source_name)
                .unwrap()
                .get_values::<f64, _>((start, count))
                .unwrap();
            let exported = subset
                .variable(&format!("product_0000/{output_name}"))
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap();
            assert_eq!(exported, source, "{output_name} coordinate differs");
        }
        for name in ["u10", "v10"] {
            let source = original
                .variable(name)
                .unwrap()
                .get_values::<f64, _>((vec![24, 4, 4], vec![168, 13, 9]))
                .unwrap();
            let exported = subset
                .variable(&format!("product_0000/{name}"))
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap();
            assert_eq!(exported, source, "{name} values differ");
        }
        subset.close().unwrap();
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir(output_dir).unwrap();
    }
    #[test]
    fn gome2_four_dimensional_radiance_exports_all_channels() {
        let input = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/satellite/gome2_band3_20130106_gome.nc");
        let output_dir = directory();
        let output = output_dir.join("gome2_channels.nc");
        let variable = inspect_subset_product(&input)
            .unwrap()
            .into_iter()
            .find(|choice| choice.path.ends_with("/OBSERVATIONS/radiance"))
            .expect("GOME-2 radiance should be discoverable");
        assert_eq!(variable.shape, vec![1, 625, 32, 64]);
        let preview = preview_subset_coverage(&[input.clone()], &variable, 1_000).unwrap();
        assert!(preview.sampled_points > 0);
        export_subset(&SubsetRequest {
            products: vec![input.clone()],
            variables: vec![variable.clone()],
            bounds: SubsetBounds {
                west: -180.0,
                south: -90.0,
                east: 180.0,
                north: 90.0,
            },
            from_date: "2013-01-06".into(),
            through_date: "2013-01-06".into(),
            mode: SubsetMode::Native,
            mosaic_method: MosaicMethod::Mean,
            series_group: String::new(),
            series_dimension: "product".into(),
            resolution: 1.0,
            output: output.clone(),
            geolocation_rules: Vec::new(),
        })
        .unwrap();
        let source = netcdf::open(input).unwrap();
        let result = netcdf::open(&output).unwrap();
        let window = spatial_window(
            &source,
            &variable,
            SubsetBounds {
                west: -180.0,
                south: -90.0,
                east: 180.0,
                north: 90.0,
            },
        )
        .unwrap()
        .unwrap();
        let (r0, r1) = (window.row_start, window.row_end);
        assert_eq!(window.columns.len(), 1);
        let run = &window.columns[0];
        let (c0, c1) = (run.source, run.source + run.count);
        let original = source
            .variable(&variable.path)
            .unwrap()
            .get_values::<f64, _>((vec![0, r0, c0, 0], vec![1, r1 - r0, c1 - c0, 64]))
            .unwrap();
        let exported = result
            .variable("product_0000/radiance")
            .unwrap()
            .get_values::<f64, _>(..)
            .unwrap();
        assert_eq!(exported.len(), (r1 - r0) * (c1 - c0) * 64);
        let mut compared = 0;
        for (source, output) in original.into_iter().zip(exported) {
            if output.is_finite() {
                assert_eq!(source, output);
                compared += 1;
            }
        }
        assert!(compared > 1_000);
        result.close().unwrap();
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir(output_dir).unwrap();
    }
    #[test]
    fn gome2_radiance_combines_as_a_geolocated_swath_series() {
        let input = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/satellite/gome2_band3_20130106_gome.nc");
        let output = directory().join("gome2_series.nc");
        let variable = inspect_subset_product(&input)
            .unwrap()
            .into_iter()
            .find(|choice| choice.path.ends_with("/OBSERVATIONS/radiance"))
            .unwrap();
        let request = SubsetRequest {
            products: vec![input],
            variables: vec![variable],
            bounds: SubsetBounds {
                west: -180.0,
                south: -90.0,
                east: 180.0,
                north: 90.0,
            },
            from_date: "2013-01-06".into(),
            through_date: "2013-01-06".into(),
            mode: SubsetMode::TimeSeries,
            series_group: "swaths".into(),
            series_dimension: "product".into(),
            mosaic_method: MosaicMethod::Mean,
            resolution: 1.0,
            output: output.clone(),
            geolocation_rules: Vec::new(),
        };
        export_subset(&request).unwrap();
        let result = netcdf::open(output).unwrap();
        let radiance = result.variable("swaths/radiance").unwrap();
        assert_eq!(radiance.dimensions().len(), 4);
        assert_eq!(radiance.dimensions()[3].len(), 64);
        assert_eq!(
            result
                .variable("swaths/latitude")
                .unwrap()
                .dimensions()
                .len(),
            3
        );
        assert!(radiance
            .get_values::<f64, _>(..)
            .unwrap()
            .iter()
            .any(|value| value.is_finite()));
        assert!(result.variable("swaths/row_count").is_some());
    }
    #[test]
    fn swath_series_pads_different_scan_lengths_with_missing_coordinates() {
        let dir = directory();
        let a = dir.join("short.nc");
        let b = dir.join("long.nc");
        let output = dir.join("swaths.nc");
        swath(&a, 2, 50.0, 1.0);
        swath(&b, 3, 50.0, 2.0);
        let mut request = request(vec![a, b], output.clone(), SubsetMode::TimeSeries);
        request.series_dimension = "swath".into();
        export_subset(&request).unwrap();
        let file = netcdf::open(output).unwrap();
        assert_eq!(
            file.variable("u10")
                .unwrap()
                .dimensions()
                .iter()
                .map(|d| d.len())
                .collect::<Vec<_>>(),
            vec![2, 3, 2]
        );
        assert_eq!(
            file.variable("row_count")
                .unwrap()
                .get_values::<i32, _>(..)
                .unwrap(),
            vec![2, 3]
        );
        let latitude = file
            .variable("latitude")
            .unwrap()
            .get_values::<f64, _>(..)
            .unwrap();
        assert!(latitude[4].is_nan() && latitude[5].is_nan());
        assert!(latitude[6..].iter().all(|value| value.is_finite()));
    }
    #[test]
    fn gome2_mosaic_keeps_separate_channel_values_and_counts() {
        let input = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/satellite/gome2_band3_20130106_gome.nc");
        let source = netcdf::open(&input).unwrap();
        let variable = inspect_subset_product(&input)
            .unwrap()
            .into_iter()
            .find(|choice| choice.path.ends_with("/OBSERVATIONS/radiance"))
            .unwrap();
        let (latitude, longitude) = (0..625)
            .find_map(|row| {
                let (latitudes, longitudes) = row_coords(&source, &variable, row, 0, 32).ok()?;
                let radiance = source.variable(&variable.path)?;
                let values = data_row(&radiance, &variable, 0, row, 0, 32).ok()?;
                latitudes.into_iter().zip(longitudes).enumerate().find_map(
                    |(column, (lat, lon))| {
                        let value = values[column * 64];
                        (lat.is_finite()
                            && lon.is_finite()
                            && lat.abs() < 85.0
                            && lon.abs() < 175.0
                            && value.is_finite()
                            && value < 1.0e30)
                            .then_some((lat, lon))
                    },
                )
            })
            .unwrap();
        let output_dir = directory();
        let output = output_dir.join("gome2_mosaic.nc");
        export_subset(&SubsetRequest {
            products: vec![input],
            variables: vec![variable],
            bounds: SubsetBounds {
                west: longitude - 1.0,
                south: latitude - 1.0,
                east: longitude + 1.0,
                north: latitude + 1.0,
            },
            from_date: "2013-01-06".into(),
            through_date: "2013-01-06".into(),
            mode: SubsetMode::Mosaic,
            mosaic_method: MosaicMethod::Mean,
            series_group: String::new(),
            series_dimension: "product".into(),
            resolution: 0.5,
            output: output.clone(),
            geolocation_rules: Vec::new(),
        })
        .unwrap();
        let result = netcdf::open(&output).unwrap();
        let radiance = result.variable("radiance").unwrap();
        assert_eq!(radiance.dimensions().last().unwrap().len(), 64);
        let values = radiance.get_values::<f64, _>(..).unwrap();
        let counts = result
            .variable("radiance_count")
            .unwrap()
            .get_values::<u32, _>(..)
            .unwrap();
        assert_eq!(values.len(), counts.len());
        assert!(counts.iter().any(|&count| count > 0));
        assert!(values
            .iter()
            .zip(counts)
            .all(|(&value, count)| count == 0 || value.is_finite()));
        result.close().unwrap();
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir(output_dir).unwrap();
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

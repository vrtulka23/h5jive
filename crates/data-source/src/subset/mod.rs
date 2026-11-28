//! Reusable bounded NetCDF geographic inspection, coverage, and export.
use super::{
    geographic_coordinate_role, select_geographic_axes, CoordinateCandidate, File, Geolocation,
    GeolocationRule, ScanDirectionClass, ScanFilter, StagedOutput,
};
use netcdf::{self, AttributeValue as Attr, Variable};
use std::path::{Path, PathBuf};
mod mosaic;
mod native;
mod series;
use mosaic::*;
use native::*;
use series::*;

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
fn netcdf_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
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

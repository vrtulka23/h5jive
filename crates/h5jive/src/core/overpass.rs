//! On-demand coordinate-only overpass checks and resumable result caches.
use super::*;
use h5jive_data::{Entry, Kind};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::UNIX_EPOCH,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OverpassDiscovery {
    pub(crate) coordinate_group: String,
    pub(crate) latitude: String,
    pub(crate) longitude: String,
    /// Zero reports one result per product; positive values use path prefixes.
    pub(crate) group_depth: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct OverpassArea {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) bounds: [f64; 4], // west, south, east, north
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct OverpassGroupResult {
    pub(crate) name: String,
    pub(crate) hit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedProduct {
    size: u64,
    modified_ns: u128,
    groups: Vec<OverpassGroupResult>,
}

#[derive(Serialize, Deserialize)]
struct OverpassCache {
    version: u8,
    root: String,
    area_key: String,
    area_bounds: [f64; 4],
    discovery: OverpassDiscovery,
    products: HashMap<String, CachedProduct>,
}

pub(crate) enum OverpassMessage {
    Progress {
        done: usize,
        total: usize,
        hits: usize,
    },
    Finished {
        results: HashMap<String, Result<Vec<OverpassGroupResult>, String>>,
        cancelled: bool,
        cache_error: Option<String>,
    },
}

fn cache_path(source: &DatasetSource, area: &OverpassArea) -> PathBuf {
    // A stable small hash keeps cache names portable and avoids area-name path characters.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in area.key.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    source
        .cache_path
        .with_extension(format!("overpass-{hash:016x}.json"))
}

fn read_overpass_cache(
    source: &DatasetSource,
    area: &OverpassArea,
) -> HashMap<String, CachedProduct> {
    let Some(cache) = fs::read(cache_path(source, area))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<OverpassCache>(&bytes).ok())
    else {
        return HashMap::new();
    };
    if cache.version == 1
        && cache.root == source.root.to_string_lossy()
        && cache.area_key == area.key
        && cache.area_bounds == area.bounds
        && cache.discovery == source.overpass
    {
        cache.products
    } else {
        HashMap::new()
    }
}

/// Display saved findings without reopening products; Check validates file mtimes.
pub(crate) fn cached_overpass_results(
    source: &DatasetSource,
    products: &[Product],
    area: &OverpassArea,
) -> HashMap<String, Result<Vec<OverpassGroupResult>, String>> {
    let cache = read_overpass_cache(source, area);
    products
        .iter()
        .filter_map(|product| {
            let cached = cache.get(&product.relative_path)?;
            (cached.size == product.size)
                .then(|| (product.relative_path.clone(), Ok(cached.groups.clone())))
        })
        .collect()
}

fn write_overpass_cache(
    source: &DatasetSource,
    area: &OverpassArea,
    products: &HashMap<String, CachedProduct>,
) -> Result<(), String> {
    let path = cache_path(source, area);
    fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&OverpassCache {
        version: 1,
        root: source.root.to_string_lossy().into_owned(),
        area_key: area.key.clone(),
        area_bounds: area.bounds,
        discovery: source.overpass.clone(),
        products: products.clone(),
    })
    .map_err(|error| error.to_string())?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn fingerprint(path: &Path) -> Result<(u64, u128), String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    Ok((metadata.len(), modified_ns))
}

fn display_group(path: &str, depth: usize) -> String {
    if depth == 0 {
        return "Product".into();
    }
    path.trim_start_matches('/')
        .split('/')
        .take(depth)
        .collect::<Vec<_>>()
        .join("/")
}

fn coordinate_pairs<'a>(
    entry: &'a Entry,
    discovery: &OverpassDiscovery,
    pairs: &mut Vec<(&'a Entry, &'a Entry, String)>,
) {
    if matches!(entry.kind, Kind::Group)
        && (discovery.coordinate_group.is_empty() || entry.name == discovery.coordinate_group)
    {
        let latitude = entry
            .children
            .iter()
            .find(|child| child.name == discovery.latitude);
        let longitude = entry
            .children
            .iter()
            .find(|child| child.name == discovery.longitude);
        if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
            if let (
                Kind::Dataset {
                    shape: lat,
                    numeric: true,
                },
                Kind::Dataset {
                    shape: lon,
                    numeric: true,
                },
            ) = (&latitude.kind, &longitude.kind)
            {
                if ((lat == lon && (2..=4).contains(&lat.len()))
                    || (lat.len() == 1 && lon.len() == 1))
                    && lat.iter().chain(lon).all(|&n| n > 0)
                {
                    pairs.push((
                        latitude,
                        longitude,
                        display_group(&entry.path, discovery.group_depth),
                    ));
                }
            }
        }
    }
    for child in &entry.children {
        if matches!(child.kind, Kind::Group) {
            coordinate_pairs(child, discovery, pairs);
        }
    }
}

fn inside_area(latitude: f64, longitude: f64, bounds: [f64; 4]) -> bool {
    let [west, south, east, north] = bounds;
    if !latitude.is_finite() || !longitude.is_finite() || latitude < south || latitude > north {
        return false;
    }
    let longitude = (longitude + 180.0).rem_euclid(360.0) - 180.0;
    if west <= east {
        (west..=east).contains(&longitude)
    } else {
        longitude >= west || longitude <= east
    }
}

fn axis_has_value(
    file: &File,
    path: &str,
    length: usize,
    predicate: impl Fn(f64) -> bool,
) -> Result<bool, String> {
    for start in (0..length).step_by(32_768) {
        let values = file
            .region(path, &[start], &[32_768.min(length - start)])
            .map_err(|error| error.to_string())?;
        if values
            .data
            .iter()
            .any(|&value| values.fill_value != Some(value) && predicate(value))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Read contiguous coordinate slabs, stopping at the first pixel center in the area.
fn coordinate_pair_hits(
    file: &File,
    lat: &Entry,
    lon: &Entry,
    area: &OverpassArea,
) -> Result<bool, String> {
    let Kind::Dataset { shape, .. } = &lat.kind else {
        return Ok(false);
    };
    if shape.len() == 1 {
        let Kind::Dataset {
            shape: lon_shape, ..
        } = &lon.kind
        else {
            return Ok(false);
        };
        if !axis_has_value(file, &lat.path, shape[0], |value| {
            value.is_finite() && (area.bounds[1]..=area.bounds[3]).contains(&value)
        })? {
            return Ok(false);
        }
        return axis_has_value(file, &lon.path, lon_shape[0], |value| {
            inside_area((area.bounds[1] + area.bounds[3]) * 0.5, value, area.bounds)
        });
    }
    let rank = shape.len();
    let rows = shape[rank - 2];
    let columns = shape[rank - 1];
    let leading = shape[..rank - 2].iter().product::<usize>();
    let row_batch = (32_768 / columns).clamp(1, rows);
    let col_batch = columns.min(32_768);
    for prefix in 0..leading {
        let mut index = prefix;
        let mut start = vec![0; rank];
        let mut count = vec![1; rank];
        for axis in (0..rank - 2).rev() {
            start[axis] = index % shape[axis];
            index /= shape[axis];
        }
        for row in (0..rows).step_by(row_batch) {
            start[rank - 2] = row;
            count[rank - 2] = row_batch.min(rows - row);
            for column in (0..columns).step_by(col_batch) {
                start[rank - 1] = column;
                count[rank - 1] = col_batch.min(columns - column);
                let latitudes = file
                    .region(&lat.path, &start, &count)
                    .map_err(|error| error.to_string())?;
                let longitudes = file
                    .region(&lon.path, &start, &count)
                    .map_err(|error| error.to_string())?;
                if latitudes
                    .data
                    .iter()
                    .zip(&longitudes.data)
                    .any(|(&latitude, &longitude)| {
                        latitudes.fill_value != Some(latitude)
                            && longitudes.fill_value != Some(longitude)
                            && inside_area(latitude, longitude, area.bounds)
                    })
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// A product may contain several independently geolocated platform groups.
pub(crate) fn check_product_overpass(
    path: &Path,
    discovery: &OverpassDiscovery,
    area: &OverpassArea,
) -> Result<Vec<OverpassGroupResult>, String> {
    let file = File::open_with_options(
        path,
        OpenOptions {
            max_depth: 8,
            max_objects: 10_000,
        },
    )
    .map_err(|error| error.to_string())?;
    let mut pairs = Vec::new();
    coordinate_pairs(&file.tree, discovery, &mut pairs);
    if pairs.is_empty() {
        return Err("No compatible latitude/longitude pair in product".into());
    }
    let mut groups = BTreeMap::<String, bool>::new();
    for (lat, lon, group) in pairs {
        if groups.get(&group) == Some(&true) {
            continue;
        }
        let hit = coordinate_pair_hits(&file, lat, lon, area)?;
        groups
            .entry(group)
            .and_modify(|current| *current |= hit)
            .or_insert(hit);
    }
    Ok(groups
        .into_iter()
        .map(|(name, hit)| OverpassGroupResult { name, hit })
        .collect())
}

/// A bounded pool avoids flooding slow mounted disks with thousands of opens.
pub(crate) fn scan_overpasses(
    source: &DatasetSource,
    products: &[Product],
    area: &OverpassArea,
    sender: &mpsc::Sender<OverpassMessage>,
    cancel: &AtomicBool,
) {
    let mut cache = read_overpass_cache(source, area);
    let current: HashSet<&str> = products
        .iter()
        .map(|product| product.relative_path.as_str())
        .collect();
    cache.retain(|relative, _| current.contains(relative.as_str()));
    let mut results = HashMap::with_capacity(products.len());
    let mut pending = Vec::new();
    for product in products {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let path = source.root.join(&product.relative_path);
        match fingerprint(&path) {
            Ok((size, modified_ns)) => {
                if let Some(cached) = cache
                    .get(&product.relative_path)
                    .filter(|cached| cached.size == size && cached.modified_ns == modified_ns)
                {
                    results.insert(product.relative_path.clone(), Ok(cached.groups.clone()));
                } else {
                    pending.push((product.relative_path.clone(), size, modified_ns));
                }
            }
            Err(error) => {
                results.insert(product.relative_path.clone(), Err(error));
            }
        }
    }
    let total = products.len();
    let done = results.len();
    let hits = results
        .values()
        .filter(|result| {
            result
                .as_ref()
                .is_ok_and(|groups| groups.iter().any(|group| group.hit))
        })
        .count();
    let _ = sender.send(OverpassMessage::Progress { done, total, hits });
    let next = AtomicUsize::new(0);
    let mut cache_error = None;
    let workers = pending.len().min(
        std::thread::available_parallelism()
            .map_or(1, |count| count.get())
            .min(2),
    );
    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::sync_channel(128);
        for _ in 0..workers {
            let tx = tx.clone();
            let next = &next;
            let pending = &pending;
            scope.spawn(move || loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some((relative, size, modified_ns)) = pending.get(index) else {
                    break;
                };
                let result =
                    check_product_overpass(&source.root.join(relative), &source.overpass, area);
                if tx
                    .send((relative.clone(), *size, *modified_ns, result))
                    .is_err()
                {
                    break;
                }
            });
        }
        drop(tx);
        let mut since_write = 0;
        let mut hits = hits;
        for (relative, size, modified_ns, result) in rx {
            if let Ok(groups) = &result {
                if groups.iter().any(|group| group.hit) {
                    hits += 1;
                }
                cache.insert(
                    relative.clone(),
                    CachedProduct {
                        size,
                        modified_ns,
                        groups: groups.clone(),
                    },
                );
                since_write += 1;
            }
            results.insert(relative, result);
            if results.len() % 32 == 0 {
                let _ = sender.send(OverpassMessage::Progress {
                    done: results.len(),
                    total,
                    hits,
                });
            }
            if since_write >= 2048 {
                if let Err(error) = write_overpass_cache(source, area, &cache) {
                    cache_error = Some(error);
                }
                since_write = 0;
            }
        }
    });
    if !cache.is_empty() {
        if let Err(error) = write_overpass_cache(source, area, &cache) {
            cache_error = Some(error);
        }
    }
    let _ = sender.send(OverpassMessage::Finished {
        results,
        cancelled: cancel.load(Ordering::Relaxed),
        cache_error,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longitude_wrap_and_area_bounds_are_handled() {
        assert!(inside_area(0.0, 179.5, [170.0, -1.0, -170.0, 1.0]));
        assert!(inside_area(0.0, -179.5, [170.0, -1.0, -170.0, 1.0]));
        assert!(!inside_area(0.0, 0.0, [170.0, -1.0, -170.0, 1.0]));
        assert!(!inside_area(f64::NAN, 0.0, [-1.0, -1.0, 1.0, 1.0]));
    }

    #[test]
    fn fdr_product_reports_independent_platforms_when_fixture_is_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../mock_datasets/FDR/ESA_FDR_ATMOS__L1A_UVN__20090606_20251029T193846_v01_01.nc",
        );
        if !path.exists() {
            return;
        }
        let discovery = OverpassDiscovery {
            coordinate_group: "GEODATA".into(),
            latitude: "latitude".into(),
            longitude: "longitude".into(),
            group_depth: 1,
        };
        let area = OverpassArea {
            key: "world".into(),
            label: "World".into(),
            bounds: [-180.0, -90.0, 180.0, 90.0],
        };
        let groups = check_product_overpass(&path, &discovery, &area).unwrap();
        assert!(groups.iter().any(|group| group.name == "GOME" && group.hit));
        assert!(groups
            .iter()
            .any(|group| group.name == "SCIAMACHY" && group.hit));
    }

    #[test]
    fn repeated_check_reuses_the_coordinate_result_cache() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let product = root.join("space_observation.nc");
        let temporary = std::env::temp_dir().join(format!(
            "h5jive_overpass_test_{}_{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let source = DatasetSource {
            id: "test".into(),
            name: "Test".into(),
            root,
            format: "CF".into(),
            tags: vec![],
            product_regex: "[.]nc$".into(),
            cache_path: temporary.join("index.json"),
            overpass: OverpassDiscovery {
                coordinate_group: String::new(),
                latitude: "latitude".into(),
                longitude: "longitude".into(),
                group_depth: 0,
            },
        };
        let area = OverpassArea {
            key: "world".into(),
            label: "World".into(),
            bounds: [-180.0, -90.0, 180.0, 90.0],
        };
        let product = Product {
            filename: "space_observation.nc".into(),
            relative_path: "space_observation.nc".into(),
            year: String::new(),
            month: String::new(),
            day: String::new(),
            orbit: String::new(),
            size: fs::metadata(product).unwrap().len(),
        };
        for expected_cached in [0, 1] {
            let (sender, receiver) = mpsc::channel();
            scan_overpasses(
                &source,
                &[product.clone()],
                &area,
                &sender,
                &AtomicBool::new(false),
            );
            let initial = receiver.try_recv().unwrap();
            let OverpassMessage::Progress { done, .. } = initial else {
                panic!("missing initial progress")
            };
            assert_eq!(done, expected_cached);
            let finish = receiver.try_iter().last().unwrap();
            let OverpassMessage::Finished { results, .. } = finish else {
                panic!("missing result")
            };
            assert!(results[&product.relative_path].as_ref().unwrap()[0].hit);
        }
        fs::remove_dir_all(temporary).unwrap();
    }
}

//! Product discovery and cached indexes for registered dataset repositories.
use super::OverpassDiscovery;
use h5jive_data::{File, OpenOptions};
use regex::Regex;
use scinumtools3::Project;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::UNIX_EPOCH,
};

#[derive(Clone, Debug)]
/// A local repository and the rule used to recognize its products.
pub(crate) struct DatasetSource {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    pub format: String,
    pub tags: Vec<String>,
    pub product_regex: String,
    pub cache_path: PathBuf,
    pub overpass: OverpassDiscovery,
}

/// Resolve DIPL repositories relative to the directory containing the settings.
pub(crate) fn read_dataset_sources(
    dip: &Project,
    settings_dir: &Path,
) -> Result<Vec<DatasetSource>, Box<dyn std::error::Error>> {
    let mut sources = Vec::new();
    for (id, _) in dip.node("dataset_sources")?.entries()? {
        let field = |name: &str| {
            dip.node(&format!("dataset_sources[{id}].{name}"))?
                .as_string()
        };
        let format = field("format.name")?;
        let tags = field("tags")?
            .split(',')
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        let root = PathBuf::from(field("root")?);
        let group_depth = dip
            .node(&format!(
                "dataset_sources[{id}].format.overpass_group_depth"
            ))?
            .as_i64()?;
        sources.push(DatasetSource {
            id: id.clone(),
            name: field("name")?,
            root: if root.is_absolute() {
                root
            } else {
                settings_dir.join(root)
            },
            format,
            tags,
            product_regex: field("product_regex")?,
            cache_path: settings_dir
                .join(".cache/datasets")
                .join(format!("{id}.json")),
            overpass: OverpassDiscovery {
                coordinate_group: field("format.coordinate_group")?,
                latitude: field("format.latitude")?,
                longitude: field("format.longitude")?,
                group_depth: group_depth as usize,
            },
        });
    }
    Ok(sources)
}

#[derive(Clone, Serialize, Deserialize)]
/// Fields kept in the on-disk index for fast filtering without opening products.
pub(crate) struct Product {
    pub(crate) filename: String,
    pub(crate) relative_path: String,
    pub(crate) year: String,
    pub(crate) month: String,
    pub(crate) day: String,
    pub(crate) orbit: String,
    pub(crate) size: u64,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct IndexFile {
    version: u8,
    root: String,
    product_regex: String,
    products: Vec<Product>,
}

pub(crate) fn date_query(input: &str) -> Option<String> {
    input
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, '-' | '/' | ' '))
        .then(|| input.chars().filter(char::is_ascii_digit).collect())
}

pub(crate) fn source_matches_tags(
    source: &DatasetSource,
    selected_tag: Option<&str>,
    query: &str,
) -> bool {
    selected_tag.is_none_or(|tag| source.tags.iter().any(|candidate| candidate == tag))
        && (query.is_empty() || source.tags.iter().any(|tag| tag.contains(query)))
}

/// Return sorted product indexes while leaving the cached product vector untouched.
pub(crate) fn matching_products(
    products: &[Product],
    search: &str,
    date_search: &str,
    orbit_search: &str,
    sort_column: usize,
    descending: bool,
) -> Vec<usize> {
    let search = search.to_lowercase();
    let date_search = date_query(date_search);
    let orbit_search = orbit_search.trim();
    let mut matching: Vec<_> = products
        .iter()
        .enumerate()
        .filter_map(|(index, product)| {
            let name_matches = search.is_empty()
                || product.filename.to_lowercase().contains(&search)
                || product.relative_path.to_lowercase().contains(&search);
            let date_matches = date_search.as_ref().is_some_and(|query| {
                query.is_empty()
                    || format!("{}{}{}", product.year, product.month, product.day)
                        .starts_with(query)
            });
            let orbit_matches = orbit_search.is_empty() || product.orbit.starts_with(orbit_search);
            (name_matches && date_matches && orbit_matches).then_some(index)
        })
        .collect();
    matching.sort_unstable_by(|&a, &b| {
        let left = &products[a];
        let right = &products[b];
        let result = match sort_column {
            2 => (&left.year, &left.month, &left.day).cmp(&(&right.year, &right.month, &right.day)),
            3 => left.orbit.cmp(&right.orbit),
            4 => left.size.cmp(&right.size),
            1 => left.relative_path.cmp(&right.relative_path),
            _ => left.filename.cmp(&right.filename),
        };
        let result = if descending { result.reverse() } else { result };
        result.then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    matching
}

pub(crate) enum ScanMessage {
    Progress(usize),
    Finished(Result<(Vec<Product>, Option<String>), String>),
}

/// Read metadata only when path and filename discovery left fields empty.
pub(crate) fn read_product_metadata(path: &Path) -> Result<[String; 4], String> {
    let attributes = File::open_with_options(
        path,
        OpenOptions {
            max_depth: 1,
            max_objects: 1,
        },
    )
    .and_then(|file| file.attributes("/"))
    .map_err(|error| error.to_string())?;
    let value = |name: &str| {
        attributes
            .iter()
            .find(|attribute| attribute.name == name)
            .map_or("", |attribute| attribute.value.as_str())
    };
    let date = ["time_coverage_start", "start_time", "date_start"]
        .into_iter()
        .map(value)
        .find(|value| value.len() >= 10)
        .unwrap_or("");
    let date = date.as_bytes();
    let component = |start: usize, end: usize| {
        std::str::from_utf8(date.get(start..end).unwrap_or(&[]))
            .unwrap_or("")
            .to_owned()
    };
    Ok([
        component(0, 4),
        component(5, 7),
        component(8, 10),
        value("orbit").to_owned(),
    ])
}

pub(crate) fn cache_path(source: &DatasetSource) -> PathBuf {
    source.cache_path.clone()
}

pub(crate) fn read_cache(source: &DatasetSource) -> Option<Vec<Product>> {
    let cache: IndexFile = serde_json::from_slice(&fs::read(cache_path(source)).ok()?).ok()?;
    (cache.version == 2
        && cache.root == source.root.to_string_lossy()
        && cache.product_regex == source.product_regex)
        .then_some(cache.products)
}

/// Replace the cache after a complete scan so a partial result never looks valid.
pub(crate) fn write_cache(source: &DatasetSource, products: &[Product]) -> Result<(), String> {
    let path = cache_path(source);
    fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&IndexFile {
        version: 2,
        root: source.root.to_string_lossy().into_owned(),
        product_regex: source.product_regex.clone(),
        products: products.to_vec(),
    })
    .map_err(|error| error.to_string())?;
    let nonce = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = path.with_extension(format!("{nonce}.tmp"));
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

/// Walk a repository in the background, reporting progress and honoring cancellation.
pub(crate) fn scan_source(
    source: &DatasetSource,
    sender: &mpsc::Sender<ScanMessage>,
    cancel: &AtomicBool,
) -> Result<Vec<Product>, String> {
    let regex = Regex::new(&source.product_regex).map_err(|error| error.to_string())?;
    if !source.root.is_dir() {
        return Err(format!("Directory not found: {}", source.root.display()));
    }
    let mut products = Vec::new();
    let mut pending = vec![source.root.clone()];
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Scan cancelled".into());
        }
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return Err("Scan cancelled".into());
            }
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            visited += 1;
            if visited % 1024 == 0 {
                let _ = sender.send(ScanMessage::Progress(visited));
            }
            let relative = path
                .strip_prefix(&source.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let Some(captures) = regex.captures(&relative) else {
                continue;
            };
            let field = |name: &str| {
                captures
                    .name(name)
                    .map_or("", |capture| capture.as_str())
                    .to_owned()
            };
            let metadata = entry.metadata().map_err(|error| error.to_string())?;
            products.push(Product {
                filename: entry.file_name().to_string_lossy().into_owned(),
                relative_path: relative.clone(),
                year: field("year"),
                month: field("month"),
                day: field("day"),
                orbit: field("orbit"),
                size: metadata.len(),
            });
        }
    }
    products.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let _ = sender.send(ScanMessage::Progress(visited));
    Ok(products)
}

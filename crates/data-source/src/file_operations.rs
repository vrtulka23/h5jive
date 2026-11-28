//! Shared admission and opening policy for mutations of HDF5 files.

use crate::{error, DataError, Entry, File, Kind, Result};
use hdf5_metno::{plist::ObjectCopy, File as H5File, Group};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const READ_HANDLE_RETRIES: usize = 50;
const READ_HANDLE_RETRY_DELAY: Duration = Duration::from_millis(100);

// HDF5 handles can be shared across app workers. Serializing writes here also
// prevents two independent plugin jobs from modifying files concurrently.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub struct WritableFile {
    file: H5File,
    _lock: MutexGuard<'static, ()>,
}

impl WritableFile {
    pub fn open(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path).map_err(error)?;
        if !metadata.is_file() {
            return Err(error(format!(
                "Destination is not a regular file: {}",
                path.display()
            )));
        }
        if metadata.permissions().readonly() {
            return Err(error(format!(
                "Destination is read-only: {}",
                path.display()
            )));
        }
        let lock = WRITE_LOCK
            .lock()
            .map_err(|_| error("File write lock is poisoned"))?;
        for attempt in 0..=READ_HANDLE_RETRIES {
            match H5File::open_rw(path) {
                Ok(file) => return Ok(Self { file, _lock: lock }),
                Err(err)
                    if attempt < READ_HANDLE_RETRIES
                        && err.to_string().contains("already open for read-only") =>
                {
                    thread::sleep(READ_HANDLE_RETRY_DELAY);
                }
                Err(err) => {
                    return Err(DataError(format!(
                        "Cannot open {} for writing: {err}",
                        path.display()
                    )))
                }
            }
        }
        unreachable!()
    }
}

impl Deref for WritableFile {
    type Target = H5File;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

pub fn same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    std::fs::canonicalize(left)
        .ok()
        .zip(std::fs::canonicalize(right).ok())
        .is_some_and(|(left, right)| left == right)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hdf5ObjectKind {
    Group,
    Dataset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyItem {
    pub path: String,
    pub kind: Hdf5ObjectKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyOutcome {
    pub copied: usize,
    pub error: Option<String>,
}

/// Copy an HDF5 object with optional reference-expansion properties.
pub fn copy_hdf5_object(
    source: &H5File,
    source_path: &str,
    destination_parent: &Group,
    name: &str,
    kind: Hdf5ObjectKind,
    properties: Option<&ObjectCopy>,
) -> Result<()> {
    match kind {
        Hdf5ObjectKind::Group => source.group(source_path).and_then(|group| {
            group.copy_to_with_props(destination_parent, name, properties.map(|v| &**v), None)
        }),
        Hdf5ObjectKind::Dataset => source.dataset(source_path).and_then(|dataset| {
            dataset.copy_to_with_props(destination_parent, name, properties.map(|v| &**v), None)
        }),
    }
    .map_err(error)
}

fn collect_copy_items(entry: &Entry, items: &mut BTreeMap<String, Hdf5ObjectKind>) {
    if entry.path != "/" {
        match entry.kind {
            Kind::Group => {
                items.insert(entry.path.clone(), Hdf5ObjectKind::Group);
            }
            Kind::Dataset { .. } => {
                items.insert(entry.path.clone(), Hdf5ObjectKind::Dataset);
            }
            Kind::Other => {}
        }
    }
    for child in &entry.children {
        collect_copy_items(child, items);
    }
}

/// Return the shallowest objects present in `source` but missing in `destination`.
pub fn copy_plan(source: &File, destination: &File) -> Vec<CopyItem> {
    let mut source_items = BTreeMap::new();
    let mut destination_items = BTreeMap::new();
    collect_copy_items(&source.tree, &mut source_items);
    collect_copy_items(&destination.tree, &mut destination_items);
    let mut plan = Vec::new();
    let mut copied_groups = BTreeSet::new();
    for (path, kind) in source_items {
        if destination_items.contains_key(&path) {
            continue;
        }
        // A copied group includes every child and all its attributes.
        let mut parent = path.as_str();
        let mut inside_copied_group = false;
        while let Some((ancestor, _)) = parent.rsplit_once('/') {
            if copied_groups.contains(ancestor) {
                inside_copied_group = true;
                break;
            }
            parent = ancestor;
        }
        if inside_copied_group {
            continue;
        }
        if kind == Hdf5ObjectKind::Group {
            copied_groups.insert(path.clone());
        }
        plan.push(CopyItem { path, kind });
    }
    plan
}

fn open_copy_files(
    source_path: &Path,
    destination_path: &Path,
) -> std::result::Result<(H5File, WritableFile, ObjectCopy), String> {
    if same_file(source_path, destination_path) {
        return Err("Source and destination must be different files".into());
    }
    let source = H5File::open(source_path).map_err(|e| e.to_string())?;
    let copy_properties = ObjectCopy::try_new().map_err(|e| format!("Copy properties: {e}"))?;
    // Keep object references valid when H5Ocopy crosses file boundaries.
    let property_result = hdf5_metno::sync::sync(|| unsafe {
        hdf5_metno_sys::h5p::H5Pset_copy_object(
            copy_properties.id(),
            hdf5_metno_sys::h5o::H5O_COPY_EXPAND_REFERENCE_FLAG,
        )
    });
    if property_result < 0 {
        return Err("Could not enable HDF5 reference copying".into());
    }
    let destination = WritableFile::open(destination_path).map_err(|error| error.to_string())?;
    Ok((source, destination, copy_properties))
}

/// Copy missing objects without replacing anything already in the destination.
pub fn copy_missing_objects(
    source_path: &Path,
    destination_path: &Path,
    plan: &[CopyItem],
) -> CopyOutcome {
    let mut copied = 0;
    let result = (|| -> std::result::Result<(), String> {
        let paths = plan
            .iter()
            .map(|item| dataset_path_parts(&item.path).map_err(|error| error.to_string()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let (source, destination, copy_properties) =
            open_copy_files(source_path, destination_path)?;
        for (item, (parent_path, name)) in plan.iter().zip(paths) {
            if destination.link_exists(&item.path) {
                continue;
            }
            let parent = if destination.link_exists(parent_path) {
                destination.group(parent_path)
            } else {
                destination.create_group(parent_path)
            }
            .map_err(|e| format!("{}: destination parent: {e}", item.path))?;
            copy_hdf5_object(
                &source,
                &item.path,
                &parent,
                name,
                item.kind,
                Some(&copy_properties),
            )
            .map_err(|e| format!("{}: {e}", item.path))?;
            copied += 1;
        }
        destination.flush().map_err(|e| e.to_string())?;
        Ok(())
    })();
    CopyOutcome {
        copied,
        error: result.err(),
    }
}

fn replacement_name(parent: &Group, prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    for index in 0.. {
        let name = format!(".__h5jive_{prefix}_{}_{nonce}_{index}", std::process::id());
        if !parent.link_exists(&name) {
            return name;
        }
    }
    unreachable!()
}

/// Replace a destination object with a staged copy and restore it on failure.
pub fn replace_hdf5_object(
    source_path: &Path,
    destination_path: &Path,
    item: &CopyItem,
) -> CopyOutcome {
    let mut replaced = 0;
    let result = (|| -> std::result::Result<(), String> {
        let (parent_path, name) =
            dataset_path_parts(&item.path).map_err(|error| error.to_string())?;
        let (source, destination, copy_properties) =
            open_copy_files(source_path, destination_path)?;
        let parent = destination
            .group(parent_path)
            .map_err(|e| format!("{}: destination parent: {e}", item.path))?;
        if !parent.link_exists(name) {
            return Err(format!("{} no longer exists in the destination", item.path));
        }
        let staged = replacement_name(&parent, "stage");
        let backup = replacement_name(&parent, "previous");
        let copied = copy_hdf5_object(
            &source,
            &item.path,
            &parent,
            &staged,
            item.kind,
            Some(&copy_properties),
        );
        if let Err(error) = copied {
            if parent.link_exists(&staged) {
                let _ = parent.unlink(&staged);
            }
            return Err(format!("{}: staging copy failed: {error}", item.path));
        }
        if let Err(error) = parent.relink(name, &backup) {
            let _ = parent.unlink(&staged);
            return Err(format!(
                "{}: could not save the destination object: {error}",
                item.path
            ));
        }
        if let Err(error) = parent.relink(&staged, name) {
            let rollback = parent.relink(&backup, name);
            let _ = parent.unlink(&staged);
            return Err(match rollback {
                Ok(()) => format!("{}: replacement failed; original restored: {error}", item.path),
                Err(rollback_error) => format!(
                    "{}: replacement failed: {error}; original remains at {backup}: {rollback_error}",
                    item.path
                ),
            });
        }
        replaced = 1;
        parent.unlink(&backup).map_err(|e| {
            format!(
                "{}: replaced, but the old object remains at {backup}: {e}",
                item.path
            )
        })?;
        destination.flush().map_err(|e| e.to_string())?;
        Ok(())
    })();
    CopyOutcome {
        copied: replaced,
        error: result.err(),
    }
}

/// Validate an absolute HDF5 object path and return its parent and name.
pub fn dataset_path_parts(path: &str) -> Result<(&str, &str)> {
    let (parent, name) = path
        .rsplit_once('/')
        .ok_or_else(|| error("Use an absolute dataset path, such as /result"))?;
    if !path.starts_with('/')
        || name.is_empty()
        || path.contains("//")
        || path.split('/').any(|part| part == "." || part == "..")
    {
        return Err(error(
            "Use an absolute, nonempty dataset path without . or ..",
        ));
    }
    Ok((if parent.is_empty() { "/" } else { parent }, name))
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[test]
    fn copy_apis_reject_malformed_paths_without_opening_files() {
        let missing = Path::new("/missing-h5jive-copy-test.h5");
        for path in ["relative", "/", "/group//value", "/group/../value"] {
            let item = CopyItem {
                path: path.into(),
                kind: Hdf5ObjectKind::Dataset,
            };
            let copied = copy_missing_objects(missing, missing, &[item.clone()]);
            assert_eq!(copied.copied, 0);
            assert!(
                copied
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("absolute")),
                "{path}"
            );
            let replaced = replace_hdf5_object(missing, missing, &item);
            assert_eq!(replaced.copied, 0);
            assert!(
                replaced
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("absolute")),
                "{path}"
            );
        }
    }
}

/// A temporary sibling of an output file, removed unless published.
pub struct StagedOutput {
    path: PathBuf,
}

impl StagedOutput {
    pub fn new(destination: &Path, purpose: &str, extension: &str) -> Result<Self> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !parent.is_dir() {
            return Err(error("Output directory does not exist"));
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(error)?
            .as_nanos();
        let path = parent.join(format!(
            ".h5jive-{purpose}-{}-{stamp}.{extension}",
            std::process::id()
        ));
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Publish without replacing a file that appeared after staging began.
    pub fn publish_new(&self, destination: &Path) -> Result<()> {
        std::fs::hard_link(&self.path, destination).map_err(error)
    }
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

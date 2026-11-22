//! Shared admission and opening policy for mutations of HDF5 files.

use crate::{error, DataError, Result};
use hdf5_metno::{plist::ObjectCopy, File as H5File, Group};
use std::{
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

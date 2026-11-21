//! Shared admission and opening policy for mutations of HDF5 files.

use crate::{error, DataError, Result};
use hdf5_metno::File as H5File;
use std::{
    ops::Deref,
    path::Path,
    sync::{Mutex, MutexGuard},
    thread,
    time::Duration,
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

//! Optional, bounded cache for a decoded map radiance plane.
use h5jive_data::Region;
use std::{
    fs,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAGIC: &[u8; 8] = b"H5JMAP01";
const MAX_CACHE_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_PLANE_CELLS: usize = 2_000_000;

pub(crate) struct MapPlaneCacheKey<'a> {
    pub(crate) source: &'a Path,
    pub(crate) dataset: &'a str,
    pub(crate) time: usize,
    pub(crate) spectral: usize,
    pub(crate) rows: usize,
    pub(crate) columns: usize,
}

impl MapPlaneCacheKey<'_> {
    fn file_name(&self) -> Option<String> {
        let metadata = fs::metadata(self.source).ok()?;
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        let source = fs::canonicalize(self.source).unwrap_or_else(|_| self.source.to_path_buf());
        let identity = format!(
            "map-plane-v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            source.to_string_lossy(),
            metadata.len(),
            modified.as_nanos(),
            self.dataset,
            self.time,
            self.spectral,
            self.rows,
            self.columns,
        );
        let hash = identity.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        Some(format!("{hash:016x}.bin"))
    }

    fn cell_count(&self) -> Option<usize> {
        self.rows
            .checked_mul(self.columns)
            .filter(|&count| count <= MAX_PLANE_CELLS)
    }
}

fn cache_directory() -> Option<PathBuf> {
    if std::env::var_os("H5JIVE_MAP_CACHE").is_some_and(|value| value.to_string_lossy() == "off") {
        return None;
    }
    if let Some(path) = std::env::var_os("H5JIVE_MAP_CACHE_DIR") {
        return Some(PathBuf::from(path));
    }
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("LOCALAPPDATA")
            .map(|path| PathBuf::from(path).join("H5Jive/map-planes"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(|path| PathBuf::from(path).join("Library/Caches/H5Jive/map-planes"));
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            .map(|path| path.join("h5jive/map-planes"))
    }
}

pub(crate) fn load_map_plane(key: &MapPlaneCacheKey<'_>) -> Option<Region> {
    load_map_plane_at(&cache_directory()?, key)
}

fn load_map_plane_at(directory: &Path, key: &MapPlaneCacheKey<'_>) -> Option<Region> {
    let count = key.cell_count()?;
    let path = directory.join(key.file_name()?);
    let mut reader = BufReader::new(fs::File::open(path).ok()?);
    let mut magic = [0_u8; 8];
    reader.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }
    let read_u64 = |reader: &mut BufReader<fs::File>| {
        let mut bytes = [0_u8; 8];
        reader.read_exact(&mut bytes).ok()?;
        Some(u64::from_le_bytes(bytes))
    };
    if read_u64(&mut reader)? != key.rows as u64 || read_u64(&mut reader)? != key.columns as u64 {
        return None;
    }
    let mut has_fill = [0_u8; 1];
    reader.read_exact(&mut has_fill).ok()?;
    let fill = f64::from_bits(read_u64(&mut reader)?);
    let byte_count = count.checked_mul(8)?;
    if reader.get_ref().metadata().ok()?.len() != 33_u64 + byte_count as u64 {
        return None;
    }
    let mut bytes = vec![0_u8; byte_count];
    reader.read_exact(&mut bytes).ok()?;
    let data = bytes
        .chunks_exact(8)
        .map(|chunk| f64::from_le_bytes(chunk.try_into().unwrap()))
        .collect();
    Some(Region {
        data,
        shape: vec![key.rows, key.columns],
        fill_value: (has_fill[0] == 1).then_some(fill),
    })
}

pub(crate) fn store_map_plane(key: &MapPlaneCacheKey<'_>, plane: &Region) {
    let Some(directory) = cache_directory() else {
        return;
    };
    let _ = store_map_plane_at(&directory, key, plane);
}

fn store_map_plane_at(
    directory: &Path,
    key: &MapPlaneCacheKey<'_>,
    plane: &Region,
) -> std::io::Result<()> {
    if key.cell_count() != Some(plane.data.len()) || plane.shape != [key.rows, key.columns] {
        return Ok(());
    }
    let Some(name) = key.file_name() else {
        return Ok(());
    };
    fs::create_dir_all(directory)?;
    let destination = directory.join(name);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    let temporary = destination.with_extension(format!("{}.{}.tmp", std::process::id(), nonce));
    let mut writer = BufWriter::new(fs::File::create(&temporary)?);
    writer.write_all(MAGIC)?;
    writer.write_all(&(key.rows as u64).to_le_bytes())?;
    writer.write_all(&(key.columns as u64).to_le_bytes())?;
    writer.write_all(&[u8::from(plane.fill_value.is_some())])?;
    writer.write_all(&plane.fill_value.unwrap_or(0.0).to_bits().to_le_bytes())?;
    for value in &plane.data {
        writer.write_all(&value.to_le_bytes())?;
    }
    writer.flush()?;
    drop(writer);
    #[cfg(target_os = "windows")]
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(temporary, destination)?;
    prune_cache(directory);
    Ok(())
}

fn prune_cache(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "bin")
        })
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            Some((entry.path(), metadata.len(), metadata.modified().ok()?))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    files.sort_by_key(|(_, _, modified)| *modified);
    for (path, size, _) in files {
        if total <= MAX_CACHE_BYTES {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total -= size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_cache_round_trips_and_invalidates_changed_source() {
        let directory =
            std::env::temp_dir().join(format!("h5jive-map-cache-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("source.nc");
        fs::write(&source, b"source").unwrap();
        let key = MapPlaneCacheKey {
            source: &source,
            dataset: "/radiance",
            time: 0,
            spectral: 2,
            rows: 2,
            columns: 2,
        };
        let plane = Region {
            data: vec![1.0, -999.0, f64::NAN, 4.0],
            shape: vec![2, 2],
            fill_value: Some(-999.0),
        };
        store_map_plane_at(&directory, &key, &plane).unwrap();
        let loaded = load_map_plane_at(&directory, &key).unwrap();
        assert_eq!(loaded.fill_value, plane.fill_value);
        assert!(loaded.data[2].is_nan());
        assert_eq!(loaded.data[0], 1.0);
        fs::write(&source, b"source changed").unwrap();
        assert!(load_map_plane_at(&directory, &key).is_none());
        fs::remove_dir_all(directory).unwrap();
    }
}

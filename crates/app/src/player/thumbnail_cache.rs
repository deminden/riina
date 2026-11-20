//! Bounded per-source disk storage for timeline preview JPEGs.
use riina_mpv::thumbnail_generator::SAMPLE_COUNT;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const CACHE_VERSION: u32 = 1;
pub(crate) type CachedFrames = Vec<(f64, Vec<u8>)>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    source_path: String,
    source_length: u64,
    source_modified_ns: u128,
    pixel_width: u32,
    timestamps: Vec<f64>,
}

#[derive(Debug)]
struct SourceKey {
    canonical_path: String,
    source_length: u64,
    source_modified_ns: u128,
    pixel_width: u32,
    digest: String,
}

impl SourceKey {
    fn new(path: &Path, pixel_width: u32) -> io::Result<Self> {
        let canonical_path = fs::canonicalize(path)?;
        let metadata = fs::metadata(&canonical_path)?;
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let canonical_path = canonical_path.to_string_lossy().into_owned();
        let source_length = metadata.len();
        let mut digest = Sha256::new();
        digest.update(canonical_path.as_bytes());
        digest.update(source_length.to_le_bytes());
        digest.update(modified.to_le_bytes());
        digest.update(pixel_width.to_le_bytes());
        let digest = digest
            .finalize()
            .iter()
            .fold(String::with_capacity(64), |mut text, byte| {
                use std::fmt::Write as _;
                let _ = write!(text, "{byte:02x}");
                text
            });
        Ok(Self {
            canonical_path,
            source_length,
            source_modified_ns: modified,
            pixel_width,
            digest,
        })
    }

    fn manifest(&self, timestamps: Vec<f64>) -> Manifest {
        Manifest {
            version: CACHE_VERSION,
            source_path: self.canonical_path.clone(),
            source_length: self.source_length,
            source_modified_ns: self.source_modified_ns,
            pixel_width: self.pixel_width,
            timestamps,
        }
    }

    fn matches(&self, manifest: &Manifest) -> bool {
        manifest.version == CACHE_VERSION
            && manifest.source_path == self.canonical_path
            && manifest.source_length == self.source_length
            && manifest.source_modified_ns == self.source_modified_ns
            && manifest.pixel_width == self.pixel_width
            && manifest.timestamps.len() == SAMPLE_COUNT as usize + 1
            && manifest
                .timestamps
                .iter()
                .all(|time| time.is_finite() && *time >= 0.0)
    }
}

pub(crate) fn load(
    root: &Path,
    source: &Path,
    pixel_width: u32,
    maximum_cache_size_mb: u64,
) -> io::Result<Option<CachedFrames>> {
    if maximum_cache_size_mb == 0 {
        return Ok(None);
    }
    if let Ok(metadata) = fs::symlink_metadata(root) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Ok(None);
        }
        evict(root, maximum_cache_size_mb.saturating_mul(1024 * 1024))?;
    } else {
        return Ok(None);
    }
    let key = SourceKey::new(source, pixel_width)?;
    let entry = root.join(&key.digest);
    match fs::symlink_metadata(&entry) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let manifest_path = entry.join("manifest.json");
    let manifest_bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let Ok(manifest) = serde_json::from_slice::<Manifest>(&manifest_bytes) else {
        return Ok(None);
    };
    if !key.matches(&manifest) {
        return Ok(None);
    }
    let mut frames = Vec::with_capacity(manifest.timestamps.len());
    for (index, timestamp) in manifest.timestamps.into_iter().enumerate() {
        let frame_path = entry.join(format!("{index:03}.jpg"));
        let bytes = match fs::read(frame_path) {
            Ok(bytes) if !bytes.is_empty() => bytes,
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        frames.push((timestamp, bytes));
    }
    // Rewriting the tiny manifest refreshes this entry's eviction age on a hit.
    if let Ok(mut file) = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(manifest_path)
    {
        let _ = file.write_all(&manifest_bytes);
    }
    Ok(Some(frames))
}

pub(crate) fn store(
    root: &Path,
    source: &Path,
    pixel_width: u32,
    maximum_cache_size_mb: u64,
    frames: &CachedFrames,
) -> io::Result<()> {
    if maximum_cache_size_mb == 0 || frames.len() != SAMPLE_COUNT as usize + 1 {
        return Ok(());
    }
    let key = SourceKey::new(source, pixel_width)?;
    fs::create_dir_all(root)?;
    let root_meta = fs::symlink_metadata(root)?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "thumbnail cache root must be a regular directory",
        ));
    }
    let entry = root.join(&key.digest);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = root.join(format!(
        ".{}.{}-{nonce}.tmp",
        key.digest,
        std::process::id()
    ));
    fs::create_dir(&staging)?;
    let result = (|| {
        let mut timestamps = Vec::with_capacity(frames.len());
        for (index, (timestamp, bytes)) in frames.iter().enumerate() {
            if !timestamp.is_finite() || *timestamp < 0.0 || bytes.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "thumbnail cache received an invalid frame",
                ));
            }
            fs::write(staging.join(format!("{index:03}.jpg")), bytes)?;
            timestamps.push(*timestamp);
        }
        let manifest = serde_json::to_vec(&key.manifest(timestamps)).map_err(io::Error::other)?;
        fs::write(staging.join("manifest.json"), manifest)?;
        if entry.exists() {
            remove_entry(&entry)?;
        }
        fs::rename(&staging, &entry)?;
        Ok(())
    })();
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    evict(root, maximum_cache_size_mb.saturating_mul(1024 * 1024))
}

fn evict(root: &Path, limit: u64) -> io::Result<()> {
    let mut entries = Vec::new();
    let mut total = 0_u64;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.len() != 64 || !name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let mut size = 0_u64;
        for child in fs::read_dir(&path)? {
            let child = child?;
            let child_meta = fs::symlink_metadata(child.path())?;
            if child_meta.is_file() && !child_meta.file_type().is_symlink() {
                size = size.saturating_add(child_meta.len());
            }
        }
        total = total.saturating_add(size);
        entries.push((metadata.modified().unwrap_or(UNIX_EPOCH), path, size));
    }
    entries.sort_by_key(|(modified, _, _)| *modified);
    for (_, path, size) in entries {
        if total <= limit {
            break;
        }
        remove_entry(&path)?;
        total = total.saturating_sub(size);
    }
    Ok(())
}

fn remove_entry(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "thumbnail cache entry must be a regular directory",
        ));
    }
    fs::remove_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::{load, store};
    use std::{fs, path::PathBuf, time::SystemTime};

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "riina-thumbnail-cache-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn cache_round_trip_is_bound_to_source_and_width() {
        let temp = TempTree::new();
        let source = temp.0.join("clip.mkv");
        fs::write(&source, b"fixture media identity").unwrap();
        let frames = (0..=100)
            .map(|index| (f64::from(index), vec![index as u8, 4, 9]))
            .collect::<Vec<_>>();
        store(&temp.0.join("cache"), &source, 240, 500, &frames).unwrap();
        assert_eq!(
            load(&temp.0.join("cache"), &source, 240, 500).unwrap(),
            Some(frames)
        );
        assert_eq!(
            load(&temp.0.join("cache"), &source, 320, 500).unwrap(),
            None
        );
        assert_eq!(load(&temp.0.join("cache"), &source, 240, 0).unwrap(), None);
    }

    #[test]
    fn cache_rejects_incomplete_entries_without_returning_partial_frames() {
        let temp = TempTree::new();
        let source = temp.0.join("clip.mkv");
        let root = temp.0.join("cache");
        fs::write(&source, b"fixture media identity").unwrap();
        let frames = (0..=100)
            .map(|index| (f64::from(index), vec![index as u8, 4, 9]))
            .collect::<Vec<_>>();
        store(&root, &source, 240, 500, &frames).unwrap();
        let entry = fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
        fs::remove_file(entry.join("050.jpg")).unwrap();
        assert_eq!(load(&root, &source, 240, 500).unwrap(), None);
    }

    #[test]
    fn cache_budget_evicts_an_entry_that_exceeds_the_limit() {
        let temp = TempTree::new();
        let source = temp.0.join("clip.mkv");
        let root = temp.0.join("cache");
        fs::write(&source, b"fixture media identity").unwrap();
        let frames = (0..=100)
            .map(|index| (f64::from(index), vec![index as u8; 12 * 1024]))
            .collect::<Vec<_>>();
        store(&root, &source, 240, 1, &frames).unwrap();
        assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    }
}

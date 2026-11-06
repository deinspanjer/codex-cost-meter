//! User-owned catalog loading and bounded local file replacement.
use crate::pricing::{Catalog, PricingError, catalog::Overlay};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

const PREFIX: &str = ".model-prices.tmp-";
const MAX_BYTES: u64 = 8 * 1024 * 1024;

pub(crate) fn path() -> Result<PathBuf, PricingError> {
    let home = crate::cli::user_home().map_err(|_| PricingError::External("cannot locate platform user home for pricing; set HOME (Unix) or USERPROFILE / HOMEDRIVE and HOMEPATH (Windows)".into()))?;
    Ok(home.join(".codex-cost-meter/model-prices.json"))
}

pub(crate) fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    use std::io::Read;
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "pricing file exceeds 8 MiB",
        ));
    }
    Ok(Some(bytes))
}

pub(crate) struct Snapshot {
    pub path: PathBuf,
    pub original: Option<Vec<u8>>,
    pub embedded: Catalog,
    pub overlay: Overlay,
    pub effective: Catalog,
}
impl Snapshot {
    pub(crate) fn load(path: PathBuf) -> Result<Self, PricingError> {
        // Cleanup is opportunistic; unrelated temp-file races must not block valuation.
        let _ = cleanup(&path);
        let original = read(&path).map_err(|e| external(&path, e))?;
        let embedded = Catalog::embedded()?;
        let overlay = match original.as_ref() {
            Some(b) => Overlay::parse(std::str::from_utf8(b).map_err(|e| external(&path, e))?)
                .map_err(|e| external(&path, e))?,
            None => Overlay::default(),
        };
        let effective = if original.is_some() {
            embedded
                .merge_overlay(&overlay)
                .map_err(|e| external(&path, e))?
        } else {
            embedded.clone()
        };
        Ok(Self {
            path,
            original,
            embedded,
            overlay,
            effective,
        })
    }
}
fn external(path: &Path, error: impl std::fmt::Display) -> PricingError {
    PricingError::External(format!(
        "{}: {error}. Delete this external file or regenerate it; pricing reset --apply can remove an incompatible file",
        path.display()
    ))
}

pub(crate) fn cleanup(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let entries = match fs::read_dir(parent) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        if !entry.file_name().to_string_lossy().starts_with(PREFIX)
            || !entry.file_type().is_ok_and(|t| t.is_file())
        {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age > Duration::from_secs(3600))
        {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}
fn check(path: &Path, original: &Option<Vec<u8>>) -> io::Result<()> {
    if &read(path)? != original {
        return Err(io::Error::other(
            "pricing file changed since review; rerun the command",
        ));
    }
    Ok(())
}
pub(crate) fn read_for_reset(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub(crate) fn reset(path: &Path, original: &Option<Vec<u8>>) -> io::Result<()> {
    if &read_for_reset(path)? != original {
        return Err(io::Error::other(
            "pricing file changed since review; rerun the command",
        ));
    }
    if original.is_some() {
        fs::remove_file(path)?;
    }
    Ok(())
}
pub(crate) fn replace(path: &Path, original: &Option<Vec<u8>>, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing pricing directory"))?;
    fs::create_dir_all(parent)?;
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut attempt = 0u32;
    let (temp, mut file) = loop {
        let temp = parent.join(format!("{PREFIX}{}-{stamp}-{attempt}", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(f) => break (temp, f),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                attempt = attempt
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("temp names exhausted"))?
            }
            Err(e) => return Err(e),
        }
    };
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        check(path, original)?;
        fs::rename(&temp, path)
    })();
    let _ = fs::remove_file(temp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_and_reset_reject_changed_destination_without_losing_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-prices.json");
        fs::write(&path, b"old").unwrap();
        let original = read(&path).unwrap();
        replace(&path, &original, b"new").unwrap(); // Also exercises Windows replacement of an existing file in CI.
        assert_eq!(read(&path).unwrap().unwrap(), b"new");
        assert!(replace(&path, &original, b"stale").is_err());
        assert!(reset(&path, &original).is_err());
        assert_eq!(read(&path).unwrap().unwrap(), b"new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        reset(&path, &read(&path).unwrap()).unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn reset_removes_oversized_invalid_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-prices.json");
        fs::File::create(&path)
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        assert!(read(&path).is_err());
        let original = read_for_reset(&path).unwrap();
        reset(&path, &original).unwrap();
        assert!(!path.exists());
    }
    #[cfg(unix)]
    #[test]
    fn unreadable_cleanup_directory_does_not_block_readable_catalog() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-prices.json");
        fs::write(&path, serde_json::to_vec(&Overlay::default()).unwrap()).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o111)).unwrap();
        let loaded = Snapshot::load(path);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(loaded.is_ok());
    }
    #[test]
    fn cleanup_only_removes_old_owned_temps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-prices.json");
        let old = dir.path().join(format!("{PREFIX}old"));
        fs::write(&old, b"temp").unwrap();
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(3601)),
            )
            .unwrap();
        for name in [
            "model-prices.json",
            "proposal.json",
            ".model-prices.tmp-recent",
        ] {
            fs::write(dir.path().join(name), b"keep").unwrap();
        }
        cleanup(&path).unwrap();
        assert!(!old.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    }
    #[test]
    fn snapshot_freezes_external_data_and_invalid_data_is_actionable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-prices.json");
        let mut overlay = Overlay {
            base: Some("none".into()),
            label: Some("My pennies".into()),
            ..Overlay::default()
        };
        fs::write(&path, serde_json::to_vec(&overlay).unwrap()).unwrap();
        let snapshot = Snapshot::load(path.clone()).unwrap();
        overlay.label = Some("changed".into());
        fs::write(&path, serde_json::to_vec(&overlay).unwrap()).unwrap();
        assert_eq!(
            snapshot.effective.metadata().label.as_deref(),
            Some("My pennies")
        );
        fs::write(&path, b"invalid").unwrap();
        assert!(
            Snapshot::load(path.clone())
                .err()
                .unwrap()
                .to_string()
                .contains("regenerate")
        );
        reset(&path, &read(&path).unwrap()).unwrap();
        assert_eq!(
            Snapshot::load(path).unwrap().effective.metadata().mode,
            "embedded"
        );
    }
}

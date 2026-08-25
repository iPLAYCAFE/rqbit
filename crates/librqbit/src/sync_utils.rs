use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use librqbit_core::torrent_metainfo::TorrentMetaV1Info;
use tracing::{info, warn};

/// Build the set of expected file paths from the torrent metadata.
fn build_expected_files(info: &TorrentMetaV1Info<buffers::ByteBufOwned>) -> HashSet<PathBuf> {
    let mut expected_files: HashSet<PathBuf> = HashSet::new();
    if let Some(files) = &info.files {
        for file in files {
            let mut path = PathBuf::new();
            for component in &file.path {
                path.push(&*bytes_to_osstr(&component.0));
            }
            expected_files.insert(path);
        }
    } else if let Some(name) = &info.name {
        let name_str = String::from_utf8_lossy(&name.0);
        expected_files.insert(PathBuf::from(name_str.as_ref()));
    }
    expected_files
}

/// Delete specific extra files from the torrent directory.
/// Returns (removed_count, failed_count).
pub fn delete_extra_files(root_path: &Path, files: &[String]) -> (usize, usize) {
    let mut removed = 0;
    let mut failed = 0;
    for file in files {
        let full_path = root_path.join(file);
        match std::fs::remove_file(&full_path) {
            Ok(_) => {
                info!("Removed extra file: {:?}", file);
                removed += 1;
            }
            Err(e) => {
                warn!("Failed to remove file {:?}: {:?}", full_path, e);
                failed += 1;
            }
        }
    }
    (removed, failed)
}

pub fn remove_extra_files(
    info: &TorrentMetaV1Info<buffers::ByteBufOwned>,
    root_path: &Path,
) -> anyhow::Result<()> {
    if !root_path.exists() {
        return Ok(());
    }

    let expected_files = build_expected_files(info);

    for entry in walkdir::WalkDir::new(root_path)
        .min_depth(1)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path == root_path {
            continue;
        }

        let relative = match path.strip_prefix(root_path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if entry.file_type().is_dir() {
            if std::fs::remove_dir(path).is_ok() {
                info!("Removed empty directory: {:?}", relative);
            }
        } else {
            if !expected_files.contains(relative) {
                info!("Removing extra file: {:?}", relative);
                if let Err(e) = std::fs::remove_file(path) {
                    warn!("Failed to remove file {:?}: {:?}", path, e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
fn bytes_to_osstr(b: &[u8]) -> std::borrow::Cow<'_, OsStr> {
    // This is a simplification. Real world torrents might have encoding mess.
    // We assume UTF-8 for valid filenames here since we are in Rust world.
    // If it fails, we fall back to lossy.
    use std::ffi::OsString;
    let s = String::from_utf8_lossy(b).into_owned();
    Cow::Owned(OsString::from(s))
}

#[cfg(unix)]
fn bytes_to_osstr(b: &[u8]) -> std::borrow::Cow<'_, OsStr> {
    use std::os::unix::ffi::OsStrExt;
    std::borrow::Cow::Borrowed(OsStr::from_bytes(b))
}

#[cfg(not(any(unix, windows)))]
fn bytes_to_osstr(b: &[u8]) -> std::borrow::Cow<'_, OsStr> {
    // Fallback for other OSes
    use std::ffi::OsString;
    let s = String::from_utf8_lossy(b).into_owned();
    Cow::Owned(OsString::from(s))
}

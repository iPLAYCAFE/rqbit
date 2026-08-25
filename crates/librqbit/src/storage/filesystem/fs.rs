use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::IoSlice,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use lru::LruCache;
use parking_lot::Mutex;
use tracing::{debug, info, warn};

use crate::{
    file_info::FileInfo,
    storage::{StorageFactoryExt, filesystem::opened_file::OurFileExt},
    torrent_state::{ManagedTorrentShared, TorrentMetadata},
};

use crate::storage::{StorageFactory, TorrentStorage};

const DEFAULT_FILE_CACHE_CAPACITY: usize = 128;

#[derive(Default, Clone, Copy)]
pub struct FilesystemStorageFactory {}

impl StorageFactory for FilesystemStorageFactory {
    type Storage = FilesystemStorage;

    fn create(
        &self,
        shared: &ManagedTorrentShared,
        _metadata: &TorrentMetadata,
    ) -> anyhow::Result<FilesystemStorage> {
        let permissive = shared.options.permissive_file_opening.unwrap_or(false);

        // When permissive_file_opening is enabled, disable the file-handle cache
        // entirely (None). With a cache, even capacity=1 keeps one handle alive,
        // and on Windows an open handle inside a directory prevents the directory
        // from being renamed/moved/deleted.
        let file_cache = if permissive {
            None
        } else {
            Some(LruCache::new(
                NonZeroUsize::new(DEFAULT_FILE_CACHE_CAPACITY).unwrap(),
            ))
        };

        Ok(FilesystemStorage {
            output_folder: shared.options.output_folder.clone(),
            allow_overwrite: shared.options.allow_overwrite,
            file_infos: Vec::new(),
            file_cache: Mutex::new(file_cache),
            permissive_file_opening: permissive,
        })
    }

    fn clone_box(&self) -> crate::storage::BoxStorageFactory {
        self.boxed()
    }
}

pub struct FilesystemStorage {
    pub(super) output_folder: PathBuf,
    allow_overwrite: bool,
    /// File metadata from torrent. Stored during init() to compute paths lazily
    /// from output_folder + relative_filename on cache miss, avoiding separate
    /// path allocation per file (as suggested by @ikatson).
    file_infos: Vec<FileInfo>,
    /// LRU cache of open file handles, keyed by file_id.
    /// Each entry is `(handle, is_writable)` — the bool tracks whether the
    /// handle was opened with write access.  When a write operation requests
    /// a file whose cached handle is read-only, the stale handle is evicted
    /// and a fresh writable handle is opened.
    /// `None` when permissive_file_opening is enabled — no handles are cached
    /// so that Windows can release the parent-directory lock immediately.
    file_cache: Mutex<Option<FileHandleCache>>,
    permissive_file_opening: bool,
}

/// Cached open handles: `(file, is_writable)`.
type FileHandleCache = LruCache<usize, (Arc<File>, bool)>;

impl FilesystemStorage {
    pub(super) fn take_fs(&self) -> anyhow::Result<Self> {
        let new_cache = {
            let cache = self.file_cache.lock();
            cache.as_ref().map(|c| LruCache::new(c.cap()))
        };
        Ok(Self {
            output_folder: self.output_folder.clone(),
            allow_overwrite: self.allow_overwrite,
            file_infos: self.file_infos.clone(),
            file_cache: Mutex::new(new_cache),
            permissive_file_opening: self.permissive_file_opening,
        })
    }

    /// Get or open a file handle for the given file_id.
    ///
    /// Uses a two-phase approach to avoid holding the lock during file open:
    /// 1. Check cache under lock — if hit **and mode matches**, return Arc<File>
    /// 2. Release lock → open file (blocking) → re-acquire lock → insert
    ///
    /// If the cached handle is read-only but `write=true` is requested, the
    /// stale handle is evicted and a fresh writable handle is opened. This
    /// prevents `seek_write()` from failing with Access Denied (OS error 5)
    /// when a read-only handle from `initial_check()` is reused for writing.
    fn get_or_open(&self, file_id: usize, write: bool) -> anyhow::Result<Arc<File>> {
        // Phase 1: check cache (if caching is enabled)
        {
            let mut cache_guard = self.file_cache.lock();
            if let Some(cache) = cache_guard.as_mut()
                && let Some((file, is_writable)) = cache.get(&file_id)
            {
                if !write || *is_writable {
                    // Cache hit: either we don't need write, or the handle is writable
                    return Ok(Arc::clone(file));
                }
                // Need write access but cached handle is read-only → evict
                tracing::info!(file_id, "upgrading read-only cached handle to writable");
                cache.pop(&file_id);
            }
        }
        // Cache miss (or evicted read-only handle) — compute path lazily
        let fi = self
            .file_infos
            .get(file_id)
            .context("file_id out of range")?;
        anyhow::ensure!(!fi.attrs.padding, "cannot open padding file");
        let path = self.output_folder.join(&fi.relative_filename);

        let mode = if write { "read/write" } else { "read-only" };
        let file = self.open_file(&path, write).with_context(|| {
            format!(
                "[open_file] failed to open file #{file_id} in {mode} mode: {:?}",
                path
            )
        })?;
        let file = Arc::new(file);

        // Phase 2: insert into cache under lock (only if caching is enabled)
        {
            let mut cache_guard = self.file_cache.lock();
            if let Some(cache) = cache_guard.as_mut() {
                // Another thread may have inserted while we were opening
                if let Some((existing, existing_writable)) = cache.get(&file_id) {
                    if !write || *existing_writable {
                        return Ok(Arc::clone(existing));
                    }
                    // Our fresh writable handle wins over the stale read-only one
                    cache.pop(&file_id);
                }
                cache.put(file_id, (Arc::clone(&file), write));
            }
        }

        Ok(file)
    }

    /// Open a file with permissive sharing (Windows), read-only fallback, and sparse marking.
    fn open_file(&self, path: &Path, write: bool) -> anyhow::Result<File> {
        let mut opts = OpenOptions::new();
        opts.read(true);
        if write {
            opts.write(true).create(true).truncate(false);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
            // Prevents "file in use" errors from antivirus, Explorer, backup tools.
            // Standard practice in qBittorrent, Deluge, Transmission.
            // Addresses: #369, #120, #192
            opts.share_mode(7);
        }

        match opts.open(path) {
            Ok(f) => {
                // Mark sparse if windows
                #[cfg(windows)]
                {
                    let _ = super::sparse::mark_file_sparse(&f);
                }
                Ok(f)
            }
            Err(e) => {
                let is_access_denied = e.kind() == std::io::ErrorKind::PermissionDenied;
                let raw_os_error = e.raw_os_error();
                let is_sharing_violation = raw_os_error == Some(32); // Windows ERROR_SHARING_VIOLATION

                if (is_access_denied || is_sharing_violation) && write {
                    if self.allow_overwrite {
                        // File needs writing (download active) but is locked
                        // by another process or has restrictive permissions.
                        // Do NOT fall back to read-only — that would cache a
                        // non-writable handle and cause a delayed write failure.
                        return Err(e).with_context(|| {
                            format!(
                                "cannot open {:?} for writing (os error {}). \
                             The file may be locked by another process \
                             (game launcher, antivirus, backup tool). \
                             Try enabling 'Kill Locking Processes' in Settings → Features, \
                             or close the application that has this file open",
                                path,
                                raw_os_error.unwrap_or(0)
                            )
                        });
                    }

                    // Read-only fallback: file doesn't need writing (seeding only).
                    // Safe because completed files only need read access.
                    tracing::debug!(
                        "error opening {:?} in read/write mode: {:#}. Falling back to read-only (seeding).",
                        path,
                        e
                    );
                    let mut read_opts = OpenOptions::new();
                    read_opts.read(true);
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::OpenOptionsExt;
                        read_opts.share_mode(7);
                    }
                    read_opts
                        .open(path)
                        .with_context(|| format!("error opening {:?} in read-only mode", path))
                } else {
                    Err(e).with_context(|| format!("error opening {:?}", path))
                }
            }
        }
    }
}

impl TorrentStorage for FilesystemStorage {
    fn pread_exact(&self, file_id: usize, offset: u64, buf: &mut [u8]) -> anyhow::Result<()> {
        let file = self.get_or_open(file_id, false)?;
        file.pread_exact(offset, buf)
    }

    fn pwrite_all(&self, file_id: usize, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        let file = self.get_or_open(file_id, true)?;
        match file.pwrite_all(offset, buf) {
            Ok(()) => Ok(()),
            Err(e) => {
                let is_access_denied = e.chain().any(|cause| {
                    cause
                        .downcast_ref::<std::io::Error>()
                        .map(|io_err| io_err.raw_os_error() == Some(5))
                        .unwrap_or(false)
                });

                if is_access_denied {
                    // Evict possibly-stale handle and retry with a fresh writable one
                    tracing::warn!(
                        file_id,
                        offset,
                        size = buf.len(),
                        "pwrite_all: Access Denied, evicting cached handle and retrying"
                    );
                    {
                        let mut cache_guard = self.file_cache.lock();
                        if let Some(cache) = cache_guard.as_mut() {
                            cache.pop(&file_id);
                        }
                    }
                    drop(file);

                    let file = self.get_or_open(file_id, true)?;
                    file.pwrite_all(offset, buf).with_context(|| {
                        let path = self.file_infos.get(file_id)
                            .map(|fi| format!("{:?}", fi.relative_filename))
                            .unwrap_or_else(|| format!("<unknown file #{file_id}>"));
                        format!("[pwrite] failed (after handle eviction retry) writing {} bytes at offset {offset} to file #{file_id} ({path})", buf.len())
                    })
                } else {
                    Err(e).with_context(|| {
                        let path = self.file_infos.get(file_id)
                            .map(|fi| format!("{:?}", fi.relative_filename))
                            .unwrap_or_else(|| format!("<unknown file #{file_id}>"));
                        format!("[pwrite] failed writing {} bytes at offset {offset} to file #{file_id} ({path})", buf.len())
                    })
                }
            }
        }
    }

    fn pwrite_all_vectored(
        &self,
        file_id: usize,
        offset: u64,
        bufs: [IoSlice<'_>; 2],
    ) -> anyhow::Result<usize> {
        let file = self.get_or_open(file_id, true)?;
        let total = bufs[0].len() + bufs[1].len();
        match file.pwrite_all_vectored(offset, bufs) {
            Ok(n) => Ok(n),
            Err(e) => {
                let is_access_denied = e.chain().any(|cause| {
                    cause
                        .downcast_ref::<std::io::Error>()
                        .map(|io_err| io_err.raw_os_error() == Some(5))
                        .unwrap_or(false)
                });

                let path = self
                    .file_infos
                    .get(file_id)
                    .map(|fi| format!("{:?}", fi.relative_filename))
                    .unwrap_or_else(|| format!("<unknown file #{file_id}>"));

                if is_access_denied {
                    tracing::warn!(
                        file_id,
                        offset,
                        total,
                        "pwrite_all_vectored: Access Denied, evicting cached handle and retrying"
                    );
                    {
                        let mut cache_guard = self.file_cache.lock();
                        if let Some(cache) = cache_guard.as_mut() {
                            cache.pop(&file_id);
                        }
                    }
                    drop(file);

                    // pwrite_all_vectored consumed the IoSlices; we cannot retry
                    // the vectored call. However, get_or_open now has a fresh
                    // writable handle cached, so the NEXT write attempt from the
                    // torrent engine will succeed.
                    Err(e).with_context(|| {
                        format!("[pwrite_vec] failed writing {total} bytes at offset {offset} to file #{file_id} ({path}). \
                                 Evicted stale handle — next attempt should succeed")
                    })
                } else {
                    Err(e).with_context(|| {
                        format!("[pwrite_vec] failed writing {total} bytes at offset {offset} to file #{file_id} ({path})")
                    })
                }
            }
        }
    }

    fn remove_file(&self, file_id: usize, filename: &Path) -> anyhow::Result<()> {
        // Evict from cache before removing (if caching is enabled)
        {
            let mut cache_guard = self.file_cache.lock();
            if let Some(cache) = cache_guard.as_mut() {
                cache.pop(&file_id);
            }
        }
        Ok(std::fs::remove_file(self.output_folder.join(filename))?)
    }

    fn ensure_file_length(&self, file_id: usize, len: u64) -> anyhow::Result<()> {
        let file = self.get_or_open(file_id, true)?;
        // Skip set_len if the file already has the correct size.
        // On Windows, File::set_len() calls SetEndOfFile which updates
        // the modification timestamp even when the size is unchanged,
        // causing mtime to reset on every restart for completed torrents.
        let current_len = file
            .metadata()
            .with_context(|| format!("[ensure_len/stat] failed to stat file #{file_id}"))?
            .len();
        if current_len != len {
            file.set_len(len)
                .with_context(|| format!("[ensure_len/set_len] failed to set length {current_len} -> {len} on file #{file_id}"))?;
        }
        Ok(())
    }

    fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
        Ok(Box::new(self.take_fs()?))
    }

    fn remove_directory_if_empty(&self, path: &Path) -> anyhow::Result<()> {
        let path = self.output_folder.join(path);
        if !path.is_dir() {
            anyhow::bail!("cannot remove dir: {path:?} is not a directory")
        }
        if std::fs::read_dir(&path)?.count() == 0 {
            std::fs::remove_dir(&path).with_context(|| format!("error removing {path:?}"))
        } else {
            warn!("did not remove {path:?} as it was not empty");
            Ok(())
        }
    }

    fn init(
        &mut self,
        shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> anyhow::Result<()> {
        info!(output_folder=?self.output_folder, file_count=metadata.file_infos.len(), "initializing filesystem storage");
        let start = std::time::Instant::now();

        if shared.options.kill_locking_processes && !shared.options.is_restoring {
            #[cfg(windows)]
            {
                if let Err(e) =
                    crate::file_locking::kill_processes_locking_path(&self.output_folder, true)
                {
                    warn!("Error killing locking processes: {:#}", e);
                }
            }
        }

        let mut created_dirs: HashSet<PathBuf> = HashSet::new();

        // Ensure the root exists
        if let Ok(p) = self.output_folder.canonicalize() {
            created_dirs.insert(p);
        } else {
            std::fs::create_dir_all(&self.output_folder)?;
            if let Ok(p) = self.output_folder.canonicalize() {
                created_dirs.insert(p);
            }
        }

        for file_details in metadata.file_infos.iter() {
            if file_details.attrs.padding {
                continue;
            }

            let full_path = self.output_folder.join(&file_details.relative_filename);

            // Deduplicate create_dir_all calls
            if let Some(parent) = full_path.parent()
                && !created_dirs.contains(parent)
            {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("error creating dir {:?}", parent))?;
                created_dirs.insert(parent.to_path_buf());
            }
        }

        self.file_infos = metadata.file_infos.clone();

        debug!(
            elapsed = ?start.elapsed(),
            files = metadata.file_infos.len(),
            dirs_created = created_dirs.len(),
            cache_capacity = DEFAULT_FILE_CACHE_CAPACITY,
            "filesystem storage initialized"
        );

        Ok(())
    }

    fn list_extra_files(
        &self,
        file_infos: &[crate::file_info::FileInfo],
    ) -> anyhow::Result<Vec<PathBuf>> {
        use std::collections::HashSet;

        // Build set of known torrent file paths (relative to output_folder)
        let known_files: HashSet<PathBuf> = file_infos
            .iter()
            .filter(|fi| !fi.attrs.padding)
            .map(|fi| fi.relative_filename.clone())
            .collect();

        let mut extra_files = Vec::new();

        if !self.output_folder.exists() {
            return Ok(extra_files);
        }

        for entry in walkdir::WalkDir::new(&self.output_folder)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let full_path = entry.path();
            if let Ok(relative) = full_path.strip_prefix(&self.output_folder) {
                let relative = relative.to_path_buf();
                if !known_files.contains(&relative) {
                    extra_files.push(relative);
                }
            }
        }

        Ok(extra_files)
    }

    fn file_metadata(&self) -> anyhow::Result<Vec<Option<(std::time::SystemTime, u64)>>> {
        Ok(self
            .file_infos
            .iter()
            .map(|fi| {
                if fi.attrs.padding {
                    return None;
                }
                let path = self.output_folder.join(&fi.relative_filename);
                std::fs::metadata(&path)
                    .ok()
                    .map(|m| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len()))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use librqbit_core::torrent_metainfo::FileDetailsAttrs;
    use std::io::Write;
    use tempfile::TempDir;

    /// Create a FileInfo for testing with the given relative filename.
    fn test_file_info(relative_filename: impl Into<PathBuf>) -> FileInfo {
        FileInfo {
            relative_filename: relative_filename.into(),
            offset_in_torrent: 0,
            piece_range: 0..1,
            attrs: FileDetailsAttrs::default(),
            len: 0,
        }
    }

    /// Create a padding FileInfo for testing.
    fn test_padding_info() -> FileInfo {
        FileInfo {
            relative_filename: PathBuf::new(),
            offset_in_torrent: 0,
            piece_range: 0..1,
            attrs: FileDetailsAttrs {
                padding: true,
                ..Default::default()
            },
            len: 0,
        }
    }

    /// Helper: create a FilesystemStorage with pre-set file_infos and output_folder
    fn make_storage_with_infos(
        output_folder: PathBuf,
        file_infos: Vec<FileInfo>,
    ) -> FilesystemStorage {
        FilesystemStorage {
            output_folder,
            allow_overwrite: false,
            file_infos,
            // Tests don't use file caching (set to None)
            file_cache: Mutex::new(None),
            permissive_file_opening: false,
        }
    }

    #[test]
    fn test_file_metadata_returns_mtime_and_size_for_existing_files() {
        let td = TempDir::with_prefix("fim_test").unwrap();
        let path = td.path().join("testfile.bin");
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(&[0u8; 1024]).unwrap();
        }

        let storage = make_storage_with_infos(
            td.path().to_path_buf(),
            vec![test_file_info("testfile.bin")],
        );
        let meta = storage.file_metadata().unwrap();

        assert_eq!(meta.len(), 1);
        let (mtime, size) = meta[0].unwrap();
        assert_eq!(size, 1024);
        let elapsed = mtime.elapsed().unwrap_or_default();
        assert!(elapsed.as_secs() < 60, "mtime too old: {:?}", elapsed);
    }

    #[test]
    fn test_file_metadata_returns_none_for_missing_files() {
        let td = TempDir::with_prefix("fim_test_missing").unwrap();
        let storage = make_storage_with_infos(
            td.path().to_path_buf(),
            vec![test_file_info("nonexistent.bin")],
        );
        let meta = storage.file_metadata().unwrap();

        assert_eq!(meta.len(), 1);
        assert!(meta[0].is_none(), "should be None for missing file");
    }

    #[test]
    fn test_file_metadata_returns_none_for_padding_files() {
        let td = TempDir::with_prefix("fim_test_padding").unwrap();
        let storage = make_storage_with_infos(td.path().to_path_buf(), vec![test_padding_info()]);
        let meta = storage.file_metadata().unwrap();

        assert_eq!(meta.len(), 1);
        assert!(meta[0].is_none(), "should be None for padding file");
    }

    #[test]
    fn test_file_metadata_detects_size_change() {
        let td = TempDir::with_prefix("fim_test_size").unwrap();
        let path = td.path().join("changing.bin");
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(&[0u8; 512]).unwrap();
        }

        let storage = make_storage_with_infos(
            td.path().to_path_buf(),
            vec![test_file_info("changing.bin")],
        );
        let baseline = storage.file_metadata().unwrap();
        assert_eq!(baseline[0].unwrap().1, 512);

        // Modify the file
        {
            let mut f = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            f.write_all(&[1u8; 2048]).unwrap();
        }

        let current = storage.file_metadata().unwrap();
        assert_eq!(current[0].unwrap().1, 2048);
        assert_ne!(
            baseline[0], current[0],
            "metadata should differ after modification"
        );
    }

    #[test]
    fn test_file_metadata_empty_file_list() {
        let td = TempDir::with_prefix("fim_test_empty").unwrap();
        let storage = make_storage_with_infos(td.path().to_path_buf(), vec![]);
        let meta = storage.file_metadata().unwrap();
        assert!(meta.is_empty());
    }

    #[test]
    fn test_file_metadata_mixed_existing_and_missing() {
        let td = TempDir::with_prefix("fim_test_mixed").unwrap();
        let existing = td.path().join("exists.bin");
        {
            let mut f = File::create(&existing).unwrap();
            f.write_all(&[42u8; 100]).unwrap();
        }

        let storage = make_storage_with_infos(
            td.path().to_path_buf(),
            vec![
                test_file_info("exists.bin"),
                test_padding_info(),
                test_file_info("does_not_exist.bin"),
            ],
        );
        let meta = storage.file_metadata().unwrap();

        assert_eq!(meta.len(), 3);
        assert!(meta[0].is_some(), "existing file should have metadata");
        assert_eq!(meta[0].unwrap().1, 100);
        assert!(meta[1].is_none(), "padding should be None");
        assert!(meta[2].is_none(), "missing file should be None");
    }
}

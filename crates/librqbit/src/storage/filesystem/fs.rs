use std::{
    fs::OpenOptions,
    io::IoSlice,
    path::{Path, PathBuf},
};

use anyhow::Context;
use tracing::{debug, warn};

use crate::{
    storage::{StorageFactoryExt, filesystem::opened_file::OurFileExt},
    torrent_state::{ManagedTorrentShared, TorrentMetadata},
};

use crate::storage::{StorageFactory, TorrentStorage};

use super::opened_file::OpenedFile;

#[derive(Default, Clone, Copy)]
pub struct FilesystemStorageFactory {}

impl StorageFactory for FilesystemStorageFactory {
    type Storage = FilesystemStorage;

    fn create(
        &self,
        shared: &ManagedTorrentShared,
        _metadata: &TorrentMetadata,
    ) -> anyhow::Result<FilesystemStorage> {
        Ok(FilesystemStorage {
            output_folder: shared.options.output_folder.clone(),
            opened_files: Default::default(),
        })
    }

    fn clone_box(&self) -> crate::storage::BoxStorageFactory {
        self.boxed()
    }
}

pub struct FilesystemStorage {
    pub(crate) output_folder: PathBuf,
    pub(crate) opened_files: Vec<OpenedFile>,
}

impl FilesystemStorage {
    #[allow(dead_code)]
    pub(crate) fn take_fs(&self) -> anyhow::Result<Self> {
        Ok(Self {
            opened_files: self
                .opened_files
                .iter()
                .map(|f| f.take_clone())
                .collect::<anyhow::Result<Vec<_>>>()?,
            output_folder: self.output_folder.clone(),
        })
    }
}

impl TorrentStorage for FilesystemStorage {
    fn pread_exact(&self, file_id: usize, offset: u64, buf: &mut [u8]) -> anyhow::Result<()> {
        self.opened_files
            .get(file_id)
            .context("no such file")?
            .lock_read()?
            .pread_exact(offset, buf)
    }

    fn pwrite_all(&self, file_id: usize, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        let of = self.opened_files.get(file_id).context("no such file")?;
        #[cfg(windows)]
        return of.try_mark_sparse()?.pwrite_all(offset, buf);
        #[cfg(not(windows))]
        return of.lock_read()?.pwrite_all(offset, buf);
    }

    fn pwrite_all_vectored(
        &self,
        file_id: usize,
        offset: u64,
        bufs: [IoSlice<'_>; 2],
    ) -> anyhow::Result<usize> {
        let of = self.opened_files.get(file_id).context("no such file")?;
        #[cfg(windows)]
        return of.try_mark_sparse()?.pwrite_all_vectored(offset, bufs);
        #[cfg(not(windows))]
        return of.lock_read()?.pwrite_all_vectored(offset, bufs);
    }

    fn remove_file(&self, _file_id: usize, filename: &Path) -> anyhow::Result<()> {
        Ok(std::fs::remove_file(self.output_folder.join(filename))?)
    }

    fn ensure_file_length(&self, file_id: usize, len: u64) -> anyhow::Result<()> {
        let f = &self.opened_files.get(file_id).context("no such file")?;
        #[cfg(windows)]
        f.try_mark_sparse()?;
        Ok(f.lock_read()?.set_len(len)?)
    }

    fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
        Ok(Box::new(Self {
            opened_files: self
                .opened_files
                .iter()
                .map(|f| f.take_clone())
                .collect::<anyhow::Result<Vec<_>>>()?,
            output_folder: self.output_folder.clone(),
        }))
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
        let mut files = Vec::<OpenedFile>::new();
        for file_details in metadata.file_infos.iter() {
            let mut full_path = self.output_folder.clone();
            let relative_path = &file_details.relative_filename;
            full_path.push(relative_path);

            if file_details.attrs.padding {
                files.push(OpenedFile::new_dummy());
                continue;
            };
            std::fs::create_dir_all(full_path.parent().context("bug: no parent")?)?;
            let f = if shared.options.allow_overwrite {
                let rw_result = OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .open(&full_path);

                match rw_result {
                    Ok(f) => f,
                    Err(e) => {
                        // Read-only fallback: if a file can't be opened for writing
                        // (e.g. completed file set to read-only, or locked by another
                        // process), fall back to read-only mode. This is safe because
                        // completed files only need read access for seeding. If writing
                        // is actually needed later, the write will fail gracefully.
                        let is_perm = e.kind() == std::io::ErrorKind::PermissionDenied;
                        let is_sharing = e.raw_os_error() == Some(32); // Windows ERROR_SHARING_VIOLATION
                        if is_perm || is_sharing {
                            debug!(
                                "error opening {:?} in read/write mode: {:#}. Trying read-only.",
                                full_path, e
                            );
                            OpenOptions::new()
                                .read(true)
                                .open(&full_path)
                                .with_context(|| {
                                    format!("error opening {:?} in read-only mode", full_path)
                                })?
                        } else {
                            return Err(e).with_context(|| {
                                format!("error opening {:?} in read/write mode", full_path)
                            });
                        }
                    }
                }
            } else {
                // create_new does not seem to work with read(true), so calling this twice.
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&full_path)
                    .with_context(|| {
                        format!(
                            "error creating a new file (because allow_overwrite = false) {:?}",
                            full_path
                        )
                    })?;
                OpenOptions::new().read(true).write(true).open(&full_path)?
            };
            files.push(OpenedFile::new(full_path.clone(), f));
        }

        self.opened_files = files;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_open_readwrite_succeeds_for_normal_file() {
        let td = TempDir::with_prefix("ro_test").unwrap();
        let path = td.path().join("normal.bin");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&[42u8; 256]).unwrap();
        }

        // Should succeed in read/write mode
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let meta = f.metadata().unwrap();
        assert_eq!(meta.len(), 256);
    }

    #[cfg(unix)]
    #[test]
    fn test_readonly_fallback_on_permission_denied() {
        use std::os::unix::fs::PermissionsExt;

        let td = TempDir::with_prefix("ro_fallback").unwrap();
        let path = td.path().join("readonly.bin");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&[0xAB; 512]).unwrap();
        }

        // Make file read-only
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&path, perms).unwrap();

        // Read/write should fail
        let rw_result = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path);
        assert!(rw_result.is_err());
        let e = rw_result.unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);

        // Read-only should succeed (this is what our fallback does)
        let f = OpenOptions::new()
            .read(true)
            .open(&path)
            .unwrap();
        let meta = f.metadata().unwrap();
        assert_eq!(meta.len(), 512);

        // Restore permissions for cleanup
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_pread_works_after_readonly_fallback() {
        use std::os::unix::fs::PermissionsExt;
        use crate::storage::filesystem::opened_file::OurFileExt;

        let td = TempDir::with_prefix("ro_pread").unwrap();
        let path = td.path().join("preadable.bin");
        let data = [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&data).unwrap();
        }

        // Make read-only
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&path, perms).unwrap();

        // Open read-only (simulating our fallback)
        let f = OpenOptions::new()
            .read(true)
            .open(&path)
            .unwrap();

        // pread should work
        let mut buf = [0u8; 4];
        f.pread_exact(4, &mut buf).unwrap();
        assert_eq!(buf, [0xCA, 0xFE, 0xBA, 0xBE]);

        // Restore permissions
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    #[test]
    fn test_readonly_file_pwrite_fails_gracefully() {
        use crate::storage::filesystem::opened_file::OurFileExt;

        let td = TempDir::with_prefix("ro_pwrite_fail").unwrap();
        let path = td.path().join("nowrite.bin");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&[0u8; 64]).unwrap();
        }

        // Make read-only
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms.clone()).unwrap();

        // Open in read-only mode
        let f = OpenOptions::new()
            .read(true)
            .open(&path)
            .unwrap();

        // pwrite should fail with an error (not panic)
        let result = f.pwrite_all(0, &[1u8; 8]);
        assert!(result.is_err(), "pwrite on read-only file should fail");

        // Restore permissions
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
    }
}


/// Mark a file as sparse on NTFS.
///
/// Without this, `set_len()` pre-allocates the full file size on disk.
/// Sparse files allocate blocks only when written, which is important for:
/// - Partial downloads (only selected files use disk space)
/// - Large torrents where not all pieces are downloaded yet
///
/// Standard practice in qBittorrent, Deluge, and other torrent clients.
/// Addresses: #484
#[cfg(windows)]
pub fn mark_file_sparse(f: &std::fs::File) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows::{
        Win32::Foundation::HANDLE, Win32::System::IO::DeviceIoControl,
        Win32::System::Ioctl::FSCTL_SET_SPARSE,
    };

    let handle = HANDLE(f.as_raw_handle());

    unsafe { DeviceIoControl(handle, FSCTL_SET_SPARSE, None, 0, None, 0, None, None).is_ok() }
}

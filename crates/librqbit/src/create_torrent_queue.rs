use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::Notify;
use tokio::task::AbortHandle;

use crate::{
    create_torrent_file::{CreateTorrentOptions, CreateTorrentResult, create_torrent},
    spawn_utils::BlockingSpawner,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TorrentCreationStatus {
    Pending,
    Processing,
    Done,
    Error,
    Cancelled,
}

#[derive(Debug, Serialize)]
pub struct CreateTorrentTask {
    pub id: usize,
    pub status: TorrentCreationStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub options: CreateTorrentOptions,
    pub source_path: PathBuf,

    pub processed_bytes: u64,
    pub total_bytes: u64,

    #[serde(skip)]
    pub result: Option<CreateTorrentResult>,
    pub error: Option<String>,
    pub magnet_link: Option<String>,

    /// Handle to abort the active hashing task. Used for cancellation via `.abort()`.
    #[serde(skip)]
    pub hashing_handle: Option<AbortHandle>,
}

pub struct TorrentCreationManager {
    tasks: RwLock<HashMap<usize, Arc<RwLock<CreateTorrentTask>>>>,
    queue: RwLock<VecDeque<usize>>,
    spawner: BlockingSpawner,
    notify: Notify,
    next_id: std::sync::atomic::AtomicUsize,
    session: RwLock<Option<std::sync::Weak<crate::session::Session>>>,
}

impl TorrentCreationManager {
    pub fn new(spawner: BlockingSpawner, cancellation_token: CancellationToken) -> Arc<Self> {
        let mgr = Arc::new(Self {
            tasks: Default::default(),
            queue: Default::default(),
            spawner,
            notify: Notify::new(),
            next_id: std::sync::atomic::AtomicUsize::new(0),
            session: Default::default(),
        });

        mgr.clone().start_worker(cancellation_token);
        mgr
    }

    pub fn set_session(&self, session: std::sync::Weak<crate::session::Session>) {
        *self.session.write() = Some(session);
    }

    fn start_worker(self: Arc<Self>, cancellation_token: CancellationToken) {
        tokio::spawn(async move {
            loop {
                let task_id = tokio::select! {
                    _ = cancellation_token.cancelled() => break,
                    id = async {
                        loop {
                            if let Some(id) = self.queue.write().pop_front() {
                                return id;
                            }
                            self.notify.notified().await;
                        }
                    } => id,
                };

                let task: Arc<RwLock<CreateTorrentTask>> = {
                    let tasks = self.tasks.read();
                    match tasks.get(&task_id) {
                        Some(t) => Arc::clone(t),
                        None => continue,
                    }
                };

                // Processing
                {
                    let mut g = task.write();
                    if matches!(g.status, TorrentCreationStatus::Error) {
                        // Cancelled?
                        continue;
                    }
                    g.status = TorrentCreationStatus::Processing;
                }

                let (path, opt_name, opt_trackers, opt_piece_length) = {
                    let g = task.read();
                    (
                        g.source_path.clone(),
                        g.options.name.clone(),
                        g.options.trackers.clone(),
                        g.options.piece_length,
                    )
                };

                // Check for file locks/process usage before starting
                let check_path = path.clone();
                let lock_check =
                    tokio::task::spawn_blocking(move || check_exclusive_access(&check_path)).await;

                let lock_result = match lock_check {
                    Ok(r) => r,
                    Err(e) => Err(anyhow::anyhow!("Task spawn error: {}", e)),
                };

                if let Err(e) = lock_result {
                    let mut g = task.write();
                    g.status = TorrentCreationStatus::Error;
                    g.error = Some(e.to_string());
                    continue;
                }

                let display_name = opt_name.clone();

                // Create watch channel externally so we can poll progress during hashing
                let (progress_tx, progress_rx) = tokio::sync::watch::channel(
                    crate::create_torrent_file::CreateTorrentProgress::default(),
                );

                // Spawn a background task that updates task progress from the watch channel
                let progress_task = {
                    let task = Arc::clone(&task);
                    let mut progress_rx = progress_rx;
                    tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                result = progress_rx.changed() => {
                                    if result.is_err() {
                                        break; // sender dropped
                                    }
                                    let progress = progress_rx.borrow().clone();
                                    let mut g = task.write();
                                    g.processed_bytes = progress.hashed_bytes;
                                    g.total_bytes = progress.total_bytes;
                                }
                            }
                        }
                    })
                };

                // Build options with progress sender
                let create_opts = crate::create_torrent_file::CreateTorrentOptions {
                    name: opt_name,
                    trackers: opt_trackers,
                    piece_length: opt_piece_length,
                    progress: Some(progress_tx),
                };

                // Spawn create_torrent as a separate task so that
                // JoinHandle::abort() can cancel it at the yield point
                // inside the hashing loop.
                let spawner = self.spawner.clone();
                let hashing_path = path.clone();
                let hashing_handle = tokio::spawn(async move {
                    create_torrent(&hashing_path, create_opts, &spawner).await
                });

                // Store the handle so cancel() can abort it
                {
                    let mut g = task.write();
                    g.hashing_handle = Some(hashing_handle.abort_handle());
                }

                // Wait for the spawned task to complete
                let create_result = match hashing_handle.await {
                    Ok(inner) => inner,
                    Err(join_err) if join_err.is_cancelled() => {
                        // Task was aborted via cancel()
                        progress_task.abort();
                        let mut g = task.write();
                        if g.status != TorrentCreationStatus::Cancelled {
                            g.status = TorrentCreationStatus::Cancelled;
                            g.error = Some("cancelled".to_string());
                        }
                        continue;
                    }
                    Err(join_err) => {
                        progress_task.abort();
                        let mut g = task.write();
                        g.status = TorrentCreationStatus::Error;
                        g.error = Some(format!("task panic: {:#}", join_err));
                        continue;
                    }
                };

                // Clear the handle now that the task is done
                {
                    let mut g = task.write();
                    g.hashing_handle = None;
                }

                match create_result {
                    Ok(res) => {
                        // Stop the progress poller
                        progress_task.abort();

                        // Construct magnet link
                        let hash = hex::encode(res.info_hash().0).to_uppercase();
                        let mut magnet = format!("magnet:?xt=urn:btih:{}", hash);

                        let name_from_info = res.as_info().info.data.name.as_ref().and_then(|b| {
                            std::str::from_utf8(b.as_ref()).ok().map(|s| s.to_string())
                        });
                        let final_name = name_from_info.or(display_name);

                        if let Some(name) = &final_name {
                            magnet.push_str(&format!("&dn={}", urlencoding::encode(name)));
                        }

                        // Trackers from metainfo
                        let trackers = res
                            .as_info()
                            .iter_announce()
                            .filter_map(|b| std::str::from_utf8(b.as_ref()).ok())
                            .collect::<Vec<_>>();

                        for tr in trackers {
                            // Lowercase URL encoding for broad client compatibility
                            let encoded = urlencoding::encode(tr).to_lowercase();
                            magnet.push_str(&format!("&tr={}", encoded));
                        }

                        // Auto-add to session (if not cancelled)
                        let session = {
                            let g = self.session.read();
                            g.as_ref().and_then(|s| s.upgrade())
                        };

                        if let Some(session) = session {
                            let add =
                                crate::session::AddTorrent::from_bytes(match res.as_bytes() {
                                    Ok(b) => b,
                                    Err(_e) => {
                                        tracing::error!(
                                            "error serializing created torrent: {:#}",
                                            _e
                                        );
                                        continue;
                                    }
                                });

                            // Normalize output folder path
                            let path_lossy = path.to_string_lossy();
                            let trimmed_path_str = path_lossy.trim_end_matches(['/', '\\']);
                            let output_folder =
                                Some(trimmed_path_str.replace('/', std::path::MAIN_SEPARATOR_STR));

                            if let Some(output_folder) = output_folder {
                                tracing::info!(
                                    "Auto-Adding torrent to session. OutputFolder: {}",
                                    output_folder
                                );
                                let opts = crate::session::AddTorrentOptions {
                                    output_folder: Some(output_folder),
                                    overwrite: true,
                                    skip_initial_check: true, // files were just hashed
                                    ..Default::default()
                                };

                                if let Err(e) = session.add_torrent(add, Some(opts)).await {
                                    tracing::error!("error auto-adding torrent: {:#}", e);
                                }
                            } else {
                                tracing::warn!(
                                    "Could not determine parent directory for auto-add: {}",
                                    path.display()
                                );
                            }
                        }

                        let mut g = task.write();
                        g.status = TorrentCreationStatus::Done;
                        g.result = Some(res);
                        g.processed_bytes = g.total_bytes;
                        g.magnet_link = Some(magnet);
                    }
                    Err(e) => {
                        progress_task.abort();
                        let msg = format!("{:#}", e);
                        let mut g = task.write();
                        g.status = TorrentCreationStatus::Error;
                        g.error = Some(msg);
                    }
                }
            }
        });
    }

    pub fn enqueue(&self, path: PathBuf, options: CreateTorrentOptions) -> anyhow::Result<usize> {
        // Normalize path for comparison (basic normalization)
        let path_lossy = path.to_string_lossy().to_string();
        // Remove trailing slash for consistency
        let normalized_path_str = path_lossy.trim_end_matches(['/', '\\']);
        // Lowercase for loose comparison on Windows-like assumptions
        let search_key = normalized_path_str.to_lowercase();

        // Check for duplicates in queue
        {
            let tasks = self.tasks.read();
            for task in tasks.values() {
                let t = task.read();
                // Check against task source path
                let t_path_lossy = t.source_path.to_string_lossy();
                let t_norm = t_path_lossy.trim_end_matches(['/', '\\']).to_lowercase();

                if t_norm == search_key
                    && matches!(
                        t.status,
                        TorrentCreationStatus::Pending | TorrentCreationStatus::Processing
                    )
                {
                    tracing::warn!("Duplicate found in queue: {}", t.source_path.display());
                    anyhow::bail!("Torrent creation for this path is already in queue");
                }
            }
        }

        // Check for duplicates in session (seeding)
        if let Some(session) = self.session.read().as_ref().and_then(|s| s.upgrade()) {
            let duplicate = session.with_torrents(|torrents| {
                for (id, handle) in torrents {
                    // Check if source path and output path match
                    if let Ok(m) = handle.metadata.load().as_ref().context("no meta") {
                        // Check if the torrent save path + name points to our path
                        let name = m.info.name().map(|n| n.into_owned());
                        if let Some(name) = name {
                            let save_path = &handle.shared.options.output_folder;
                            // Reconstruct the full path where this torrent is allegedly seeding from
                            let torrent_path = save_path.join(name);

                            let tp_lossy = torrent_path.to_string_lossy();
                            let tp_norm = tp_lossy.trim_end_matches(['/', '\\']).to_lowercase();

                            // Check the direct output folder too, in case user pointed directly to the dir
                            let save_path_lossy = save_path.to_string_lossy();
                            let save_norm =
                                save_path_lossy.trim_end_matches(['/', '\\']).to_lowercase();

                            if tp_norm == search_key || save_norm == search_key {
                                tracing::warn!(
                                    "Duplicate found in session (id={}): {}",
                                    id,
                                    torrent_path.display()
                                );
                                return true;
                            }
                        }
                    }
                }
                false
            });
            if duplicate {
                anyhow::bail!("This path is already managed by the session (seeding/downloading)");
            }
        }

        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let task = Arc::new(RwLock::new(CreateTorrentTask {
            id,
            status: TorrentCreationStatus::Pending,
            created_at: chrono::Utc::now(),
            options,
            source_path: path,
            processed_bytes: 0,
            total_bytes: 0,
            result: None,
            error: None,
            magnet_link: None,
            hashing_handle: None,
        }));

        self.tasks.write().insert(id, task);
        self.queue.write().push_back(id);
        self.notify.notify_one();
        Ok(id)
    }

    pub fn get(&self, id: usize) -> Option<Arc<RwLock<CreateTorrentTask>>> {
        self.tasks.read().get(&id).cloned()
    }

    pub fn list(&self) -> Vec<Arc<RwLock<CreateTorrentTask>>> {
        let mut tasks: Vec<_> = self.tasks.read().values().cloned().collect();
        // Sort by id?
        tasks.sort_by_key(|t: &Arc<RwLock<CreateTorrentTask>>| t.read().id);
        tasks
    }

    pub fn cancel(&self, id: usize) -> anyhow::Result<()> {
        let tasks = self.tasks.read();
        let task = tasks.get(&id).context("task not found")?;
        let mut g = task.write();

        match g.status {
            TorrentCreationStatus::Pending => {
                g.status = TorrentCreationStatus::Cancelled;
                g.error = Some("cancelled".to_string());
            }
            TorrentCreationStatus::Processing => {
                // Abort the hashing task if it's running
                if let Some(handle) = g.hashing_handle.take() {
                    handle.abort();
                }
                g.status = TorrentCreationStatus::Cancelled;
                g.error = Some("cancelled".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    pub fn cleanup(&self, id: usize) {
        let mut tasks = self.tasks.write();
        tasks.remove(&id);
    }
}

fn check_exclusive_access(path: &Path) -> anyhow::Result<()> {
    // If it's a file, try to open it with write access to see if it's locked by another process
    if path.is_file() {
        if let Err(e) = std::fs::OpenOptions::new().write(true).open(path)
            && is_sharing_violation(&e)
        {
            anyhow::bail!(
                "File is locked by another process: {}. Close any programs using this file.",
                path.display()
            );
        }
        // ACCESS_DENIED or other errors (e.g. read-only attribute) are OK — the file
        // can still be read for hashing.
        return Ok(());
    }

    // If directory, check files inside recursively
    if path.is_dir() {
        let walk = walkdir::WalkDir::new(path).into_iter();
        for entry in walk.filter_entry(|e| !is_hidden(e)) {
            match entry {
                Ok(entry) => {
                    if entry.file_type().is_file()
                        && let Err(e) = std::fs::OpenOptions::new().write(true).open(entry.path())
                        && is_sharing_violation(&e)
                    {
                        anyhow::bail!(
                            "File in use or locked: {}. Close any programs using this folder.",
                            entry.path().display()
                        );
                    }
                    // Read-only or permission-denied files are fine — skip them.
                }
                Err(_) => continue,
            }
        }
    }
    Ok(())
}

/// Check if an I/O error is a Windows sharing violation (ERROR_SHARING_VIOLATION = 32),
/// meaning another process has a lock on the file.
/// On non-Windows platforms, we conservatively treat all write-open failures as potential locks.
fn is_sharing_violation(e: &std::io::Error) -> bool {
    #[cfg(windows)]
    {
        // ERROR_SHARING_VIOLATION = 32
        e.raw_os_error() == Some(32)
    }
    #[cfg(not(windows))]
    {
        // On non-Windows, we can't easily distinguish, so treat all failures as locks
        let _ = e;
        true
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

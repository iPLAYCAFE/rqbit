# rqbit (Windows Enhanced Fork)

This is a customized fork of [rqbit](https://github.com/ikatson/rqbit), focused on providing a **production-grade Windows Desktop experience** with advanced torrent creation and management features.

> **Fork Point**: [`v9.0.1`](https://github.com/ikatson/rqbit/releases/tag/v9.0.1) (`a499d2f2`) on upstream `main`
> **Delta**: **1 squashed commit** on `main` above `v9.0.1` (see `git diff upstream/main...main --stat`)
> **Repo**: [github.com/iPLAYCAFE/rqbit](https://github.com/iPLAYCAFE/rqbit) (GitHub fork of [ikatson/rqbit](https://github.com/ikatson/rqbit))
> **Agent context**: [`AGENTS.md`](AGENTS.md) · [`README.feature_strategy.md`](README.feature_strategy.md)

---

## Table of Contents

1. [New Modules (Files Added)](#1-new-modules-files-added)
2. [Core Library Changes (`crates/librqbit/src/`)](#2-core-library-changes)
3. [Storage Layer Changes](#3-storage-layer-changes)
4. [Torrent State & Lifecycle Changes](#4-torrent-state--lifecycle-changes)
5. [HTTP API Changes](#5-http-api-changes)
6. [Peer Connection & Networking Changes](#6-peer-connection--networking-changes)
7. [Session Persistence Changes](#7-session-persistence-changes)
8. [Desktop App Changes (`desktop/`)](#8-desktop-app-changes)
9. [WebUI Changes (`crates/librqbit/webui/`)](#9-webui-changes)
10. [Build & Tooling Changes](#10-build--tooling-changes)
11. [Known Pitfalls & Prevention Guidelines](#11-known-pitfalls--prevention-guidelines)
12. [Build Instructions](#12-build-instructions)
13. [Credits](#13-credits)

---

## 1. New Modules (Files Added)

These files are **entirely new** and do not exist in upstream:

| File | Purpose |
|---|---|
| `create_torrent_queue.rs` | Background torrent creation queue with worker, deduplication, progress tracking, cancellation, and auto-seeding |
| `file_locking.rs` | Windows-only process killing via Win32 Restart Manager API (`RmStartSession` / `RmShutdown`) |
| `sync_utils.rs` | Post-download cleanup: list and remove files and empty directories not present in the torrent manifest |
| `webui/src/components/buttons/CreateTorrentButton.tsx` | Header action button for creating torrents |
| `tests/e2e_permissive.rs` | End-to-end test for `FILE_SHARE_DELETE` permissive file opening |
| `webui/src/components/CopyMagnetButton.tsx` | Clipboard component with 3-tier fallback (Tauri plugin → `navigator.clipboard` → textarea) |
| `webui/src/components/modal/CreateTorrentModal.tsx` | UI for creating torrents via directory/file picker |
| `webui/src/components/modal/CreateTorrentQueueModal.tsx` | UI for monitoring the creation queue (progress, cancel, delete) |
| `webui/src/helper/formatDate.ts` | Date formatting utility for "Last Active" column |
| `webui/src/helper/magnetUtils.ts` | Magnet link construction helper with lowercase URL encoding |
| `webui/src/hooks/useDocumentVisibility.ts` | Hook for adaptive polling (reduces interval when tab is hidden) |
| `webui/src/components/modal/CleanExtraFilesModal.tsx` | Manual cleanup dialog: lists extra files, allows selective deletion with select-all/individual toggles |
| `desktop/src-tauri/build.rs` | Compile-time version injection (`RQBIT_VERSION` env var from `Cargo.toml`) |

---

## 2. Core Library Changes

### 2.1 `create_torrent_file.rs` — Torrent Creation Engine

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `CreateTorrentOptions` struct | `CreateTorrentOptions<'a>` with `name: Option<&'a str>` | `CreateTorrentOptions` (no lifetime), `name: Option<String>`, adds `Serialize`/`Deserialize` derives. Includes optional `progress: Option<watch::Sender<CreateTorrentProgress>>` field (skipped during serde) | **Changed** |
| `CreateTorrentProgress` struct | N/A | New: `total_bytes`, `hashed_bytes`, `total_files`, `hashed_files` — sent via `watch::Sender` for live progress | **Added** |
| `choose_piece_length()` | Always returns `2 * 1024 * 1024` (2 MiB) | Dynamic: aims for ~1200 pieces, power-of-2, clamped between 32 KiB – 16 MiB | **Changed** |
| `create_torrent()` signature | `(path, options, spawner)` | `(path, options, spawner)` — unchanged signature, but `options` now carries optional `progress` sender | **Changed** |
| `create_torrent_raw()` signature | Same as above | `(path, options, spawner)` — reads `options.progress` internally | **Changed** |
| Progress reporting | None | Sends `CreateTorrentProgress` via `watch::channel`: `hashed_files` incremented after each file completes, `hashed_bytes` updated at each **piece boundary** | **Added** |
| Cancellation | None | Drop-based: caller spawns as `tokio::spawn` and uses `AbortHandle::abort()`. The hashing loop calls `tokio::task::yield_now().await` at each piece boundary to create an await point where abort can take effect. | **Added** |
| `CreateTorrentResult` | `#[derive(Debug)]` | `#[derive(Debug, Clone)]` — enables reuse in queue | **Changed** |

### 2.2 `create_torrent_queue.rs` — Creation Queue System (NEW)

Entirely new module. Key types and functions:

| Item | Description |
|---|---|
| `TorrentCreationStatus` enum | States: `Pending`, `Processing`, `Done`, `Error`, `Cancelled` |
| `CreateTorrentTask` struct | Fields: `id`, `status`, `created_at`, `options`, `source_path`, `processed_bytes`, `total_bytes`, `result`, `error`, `magnet_link`, `hashing_handle: Option<AbortHandle>` (for cancellation via `.abort()`) |
| `TorrentCreationManager` struct | Owns `tasks: RwLock<HashMap>`, `queue: RwLock<VecDeque>`, `spawner`, `notify: Notify`, `next_id: AtomicUsize`, weak `Session` ref |
| `TorrentCreationManager::new()` | Creates manager and spawns background worker loop |
| `TorrentCreationManager::enqueue()` | Validates path, checks for duplicates (queue + active session), creates task, pushes to queue |
| `TorrentCreationManager::cancel()` | Calls `.abort()` on the task's `AbortHandle`; the tokio runtime drops the hashing future at the next `yield_now().await` point (piece boundary) |
| `TorrentCreationManager::cleanup()` | Removes task from HashMap |
| `TorrentCreationManager::list()` | Returns sorted task list |
| `check_exclusive_access()` | Pre-creation check: recursively write-opens files; uses `is_sharing_violation()` to distinguish Windows `ERROR_SHARING_VIOLATION` (OS error 32, actual lock → error) from `ERROR_ACCESS_DENIED` (OS error 5, read-only attribute → skipped) |
| `is_sharing_violation()` | Platform-aware helper: on Windows, checks `raw_os_error() == Some(32)`; on other platforms, conservatively treats all failures as locks |
| Worker loop | Processes queue serially: file lock check → creates `watch::channel` → spawns progress poller task → spawns hashing as `tokio::spawn` (stores `AbortHandle` for cancellation) → awaits result → aborts poller → magnet link generation → auto-add to session |
| Progress poller | Spawned per task: listens on `progress_rx.changed()` (event-driven, not polling) and updates `CreateTorrentTask.processed_bytes` / `total_bytes` in real-time; aborted on completion or error |
| Magnet link generation | Constructs `magnet:?xt=urn:btih:{hash}&dn={name}&tr={tracker}` with **lowercase URL encoding** for broad client compatibility |
| Auto-seeding | On success, auto-adds created torrent to session with `skip_initial_check: true` and normalized output folder |
| Duplicate prevention (queue) | Rejects `enqueue()` if same normalized path is already `Pending` or `Processing` |
| Duplicate prevention (session) | Rejects `enqueue()` if session already has a torrent with matching output_folder or torrent_path |

### 2.3 `file_locking.rs` — Kill Locking Processes (NEW, Windows Only)

| Item | Description |
|---|---|
| `kill_processes_locking_path(path, recursive)` | Uses Win32 Restart Manager API: `RmStartSession` → `RmRegisterResources` (all files recursively) → `RmGetList` → `RmShutdown`. Includes `SessionGuard` RAII for cleanup. Non-Windows: returns error. |

### 2.4 `sync_utils.rs` — Extra File Removal (NEW)

| Item | Description |
|---|---|
| `remove_extra_files(info, root_path)` | Walks the file system under `root_path`, compares against the torrent manifest's file list, deletes files not in the manifest, then removes newly-empty directories bottom-up. Does **not** skip hidden files — all non-manifest files are removed. |

### 2.5 `api.rs` — API Layer

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `normalize_path_string()` | N/A | Normalizes `to_string_lossy()` paths to native OS separators (fixes mixed `/` and `\` on Windows) | **Added** |
| `output_folder` fields | Used `to_string_lossy().into_owned()` | Uses `normalize_path_string()` everywhere | **Changed** |
| `TorrentDetailsResponse.trackers` | N/A | `Option<Vec<String>>` — tracker URLs in detail responses | **Added** |
| `ApiAddTorrentResponse.already_managed` | N/A | `bool` (default false) — indicates if torrent was already in session | **Added** |
| `make_torrent_details()` signature | 6 params | 7 params — adds `trackers: Option<Vec<String>>` | **Changed** |
| List endpoint optimization | Returns full `stats` | Clears `stats.file_progress`, forces `files = None` and `trackers = None` in list view | **Changed** |
| `api_create_torrent_enqueue()` | N/A | Delegates to `TorrentCreationManager::enqueue()` | **Added** |
| `api_create_torrent_list()` | N/A | Returns all tasks from creation manager | **Added** |
| `api_create_torrent_cancel()` | N/A | Cancels a creation task by ID | **Added** |
| `api_create_torrent_delete()` | N/A | Removes a completed/cancelled task | **Added** |

### 2.6 `session.rs` — Session Management

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `Session` struct fields | — | Added: `kill_locking_processes: bool`, `sync_extra_files: bool`, `skip_hash_check: bool`, `permissive_file_opening: Option<bool>`, `enable_file_integrity_monitor: bool`, `torrent_creation_manager: Arc<TorrentCreationManager>` | **Added** |
| `SessionOptions` struct | — | Added: `kill_locking_processes`, `sync_extra_files`, `skip_hash_check`, `permissive_file_opening`, `enable_file_integrity_monitor` | **Added** |
| `AddTorrentOptions` struct | — | Added: `skip_initial_check: bool`, `kill_locking_processes: Option<bool>`, `sync_extra_files: Option<bool>`, `permissive_file_opening: Option<bool>`, `added_at: Option<DateTime>`, `total_fetched_bytes: Option<u64>`, `last_activity: Option<DateTime>`, `is_restoring: bool` | **Added** |
| Duplicate output_folder check | N/A | On `add_torrent()`, checks if any existing torrent has the same normalized `output_folder`; returns `AlreadyManaged` if so | **Added** |
| Session restore behavior | No change | If `skip_hash_check` is set, forces `skip_initial_check = true` on all restored torrents; sets `is_restoring = true` | **Changed** |
| `create_and_serve_torrent()` | `(path, opts)` | `(path, opts)` — calls `create_torrent(path, opts, &self.spawner)` then auto-adds result to session with `skip_initial_check: true` and `overwrite: true` | **Changed** |
| `ManagedTorrentOptions` propagation | — | Propagates `kill_locking_processes`, `sync_extra_files`, `is_restoring`, `_skip_hash_check`, `permissive_file_opening` from `AddTorrentOptions` to per-torrent options | **Added** |
| `ManagedTorrentShared.added_at` | N/A | `Option<DateTime<Utc>>` — set during add, defaults to `Utc::now()` | **Added** |
| `ManagedTorrentLocked` struct | — | Added: `total_fetched_bytes: u64`, `last_activity: Option<DateTime>` | **Added** |
| `TorrentStateInitializing::new()` | 4 params | 5 params — adds `skip_check: bool` | **Changed** |

---

## 3. Storage Layer Changes

### 3.1 `storage/filesystem/fs.rs` — Filesystem Storage

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `FilesystemStorage` struct | `opened_files: Vec<OpenedFile>` | `file_infos: Vec<FileInfo>` + `file_cache: Mutex<Option<LruCache<usize, (Arc<File>, bool)>>>` + `allow_overwrite: bool` + `permissive_file_opening: bool` | **Changed** |
| File handle cache | N/A | `lru::LruCache` (128 capacity default). Each entry stores `(Arc<File>, bool)` where the `bool` tracks whether the handle was opened for writing. When a write operation requests a file but the cached handle is read-only, the stale handle is **evicted and re-opened** with write access. This prevents `seek_write()` from failing with Access Denied (OS error 5) when a read-only handle from `initial_check()` is reused for download writes. Wrapped in `Option` — set to `None` when permissive mode is enabled. Two-phase locking: check cache → release lock → open file → re-acquire lock → insert. | **Added** |
| `file_metadata()` | N/A | `TorrentStorage` trait method: `fn file_metadata(&self) -> anyhow::Result<Vec<Option<(SystemTime, u64)>>>`. Returns mtime + size for each file. Used by FIM for baseline capture and runtime polling. Default trait impl returns error (non-filesystem backends); `FilesystemStorage` reads `std::fs::metadata()` per file. Covered by 6 unit tests. | **Added** |
| File opening | Opens all files eagerly in `init()` | **Lazy**: stores `Vec<FileInfo>` in `init()`, computes full path lazily from `output_folder.join(&fi.relative_filename)` on cache miss; opens file on first `pread/pwrite` via LRU cache | **Changed** |
| `FILE_SHARE_DELETE` support | N/A | Opens files with `share_mode(7)` (`FILE_SHARE_READ | WRITE | DELETE`) on Windows. When `permissive_file_opening = true`, the LRU cache is set to `None` (disabled entirely) so no file handles are held open, allowing Windows to release parent-directory locks for rename/move/delete. | **Added** |
| `pread_exact()` / `pwrite_all()` | Used `OpenedFile::lock_read()` | Uses `get_or_open(file_id, write)` from cache. `pwrite_all()` includes **handle-eviction retry**: if the write fails with Access Denied (OS error 5), the cached handle is evicted, a fresh writable handle is opened, and the write is retried once. `pwrite_all_vectored()` evicts the stale handle on Access Denied but **cannot retry** the write (the `IoSlice` buffers are consumed) — instead it ensures the next write attempt from the torrent engine uses a fresh handle. | **Changed** |
| Error handling on open | Hard failure | Tries R/W mode; on `PermissionDenied` or sharing violation: if `allow_overwrite` is true (download active), **fails immediately** with descriptive error (prevents caching a non-writable handle); if `allow_overwrite` is false (seeding), falls back to read-only open | **Added** |
| Sparse file marking | In `pwrite_all()` | In `open_file()` — calls `mark_file_sparse(&f)` via `FSCTL_SET_SPARSE` ioctl immediately after successful file open (Windows only, failure silently ignored) | **Moved** |
| `init()` directory creation | `create_dir_all()` for every file | `HashSet<PathBuf>` tracks created dirs; skips redundant `create_dir_all()` syscalls | **Changed** |
| `init()` kill locking processes | N/A | If `kill_locking_processes && !is_restoring`, calls `kill_processes_locking_path()` before opening files | **Added** |
| `init()` logging | None | `debug!` log with `elapsed`, `files`, `dirs_created`, `cache_capacity` fields | **Added** |
| `take()` / `take_fs()` | Clones `OpenedFile` instances | Creates a **new empty cache** with the same capacity (via `LruCache::new(c.cap())`), clones `file_infos`, `output_folder`, and flags — does **not** carry open file handles to the new instance | **Changed** |

### 3.2 `storage/filesystem/opened_file.rs` — Low-Level Write Retry (Windows)

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `pwrite_all()` (Windows) | Single `seek_write()` loop | Retries `seek_write()` up to **3 times** with increasing backoff (100ms, 200ms, 300ms) when `ERROR_ACCESS_DENIED` (OS error 5) is encountered. Handles transient antivirus scanning, NTFS journal updates, and sparse block allocation under heavy I/O. Logs `tracing::warn` on each retry attempt and `tracing::info` on successful recovery. | **Changed** |

### 3.3 Startup Performance

| Metric | Upstream | Fork |
|---|---|---|
| Init time for 60,000+ files | >60 seconds (eager open + individual `create_dir_all`) | <150 ms (lazy open + deduped dir creation) |

---

## 4. Torrent State & Lifecycle Changes

### 4.1 `torrent_state/initializing.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `skip_check` field | N/A | `bool` — if true, skips hash check entirely; fills all pieces as "have" and returns early via `finalize_check()` | **Added** |
| `TorrentStateInitializing::new()` | 5 params | 6 params — adds `skip_check: bool` | **Changed** |
| `validate_fastresume()` method | Inline in `check()` | Validates at least one piece per file from the fast-resume bitfield, then randomly samples remaining pieces with decreasing probability (1/min(idx+1, 50)). If any piece fails hash check, clears the bitfield and forces full re-check. | **Refactored** |
| `finalize_check()` method | Inline in `check()` | Extracted to separate async method; reused by both skip_check path and normal check path. Contains `sync_extra_files` logic and `ensure_file_length` loop. | **Refactored** |
| Startup integrity validation | N/A | When `skip_check && enable_file_integrity_monitor`, performs two checks before accepting fast-resume: (1) **size check** — compares actual file sizes against `.torrent` metadata lengths; (2) **mtime check** — compares file modification times against `.bitv` file mtime. If any discrepancy is found, bails with error message instructing user to Force Recheck. Prevents serving corrupted data that was modified during a previous shutdown. | **Added** |
| `sync_extra_files` at init | N/A | In `finalize_check()`: after hash check, if `sync_extra_files && !is_restoring && torrent is finished`, calls `remove_extra_files()` | **Added** |
| `ensure_file_length()` mtime preservation | Always calls `set_len()` | In `fs.rs`: checks `file.metadata().len()` first; skips `set_len()` when file size already matches. On Windows, `set_len()` calls `SetEndOfFile` which updates the modification timestamp even when size is unchanged — this fix preserves original mtime on restart. | **Changed** |
| `ensure_file_length()` error handling | Logs warning for all errors | In `finalize_check()`: distinguishes `PermissionDenied` (debug-level, for read-only files) from other errors (warn-level) | **Changed** |

### 4.2 `torrent_state/mod.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `ManagedTorrentLocked` fields | — | Added `total_fetched_bytes: u64`, `last_activity: Option<DateTime>` | **Added** |
| `ManagedTorrentOptions` fields | — | Added `kill_locking_processes`, `sync_extra_files`, `is_restoring`, `_skip_hash_check`, `enable_file_integrity_monitor`, `permissive_file_opening` | **Added** |
| `ManagedTorrentShared.added_at` | N/A | `Option<DateTime<Utc>>` | **Added** |
| `ManagedTorrent::pause()` | Inline match arm | Extracts `live` clone first, then calls `live.pause()`. Accumulates `total_fetched_bytes` and `last_activity` from live state. | **Changed** |
| `ManagedTorrent::stats()` | Returns basic stats | Additionally returns `added_at`, `total_fetched_bytes`, `last_activity` | **Changed** |
| `ManagedTorrent::get_total_fetched_bytes()` | N/A | Returns `locked.total_fetched_bytes + live.get_downloaded_bytes()` | **Added** |
| `ManagedTorrent::get_last_activity()` | N/A | Returns stored `last_activity` from locked state | **Added** |
| Fatal error handling | Warns on pause failure | Silences pause failure; accumulates fetched_bytes and last_activity | **Changed** |

### 4.3 `torrent_state/live/mod.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `last_activity: AtomicI64` | N/A | Tracks last UTC timestamp (millis) when either `fetched_bytes` or `uploaded_bytes` changed | **Added** |
| `get_last_activity()` method | N/A | Returns `Option<DateTime<Utc>>` from the `AtomicI64` | **Added** |
| Speed snapshot loop | Tracks only download | Also tracks upload; compares both against previous snapshot to update `last_activity` | **Changed** |
| Peer pruner task | N/A | Spawns `peer_pruner` that runs every 60s, calls `prune_peers(2000)` | **Added** |
| `task_connect_to_peer()` | Hard error on `BugPeerNotFound` | Gracefully handles with debug log "peer pruned before connection attempt" and returns `Ok(())` | **Changed** |
| File Integrity Monitor (FIM) | N/A | When `enable_file_integrity_monitor` is true: captures a baseline of file metadata (mtime + size) at torrent start, then spawns a periodic monitor task that re-checks metadata against the baseline. If any file has been modified or deleted externally, calls `on_fatal_error()` to auto-pause the torrent. Uses adaptive polling intervals: 60s (≤1K files), 120s (≤10K), 180s (≤50K), 300s (>50K). **Baseline is refreshed** when download completes (`chunks.is_finished()`) so that download-caused mtime changes are not flagged as external modification. | **Added** |
| `compute_monitor_interval()` | N/A | Extracted helper function that computes the FIM polling interval based on file count. Covered by 4 unit tests. | **Added** |
| `check_file_integrity()` | N/A | Extracted comparison function (`baseline` vs `current` metadata) that returns `IntegrityCheckResult` enum (`Ok`, `FileModified`, `FilesDeleted`). Covered by 8 unit tests. | **Added** |
| `sync_extra_files` on download complete | N/A | Spawns `sync_extra_files` blocking task that calls `remove_extra_files()` when torrent finishes | **Added** |
| FATAL error context | Generic error string | Structured log fields: `piece`, `chunk_offset`, `chunk_size`, `peer` address for precise diagnosis of write failures | **Changed** |

### 4.4 `torrent_state/live/peers/mod.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `PeerStates::prune_peers(max_peers)` | N/A | Removes peers above limit; priority: `Dead`/`NotNeeded` first, then `Queued`. Returns count removed. | **Added** |

### 4.5 `torrent_state/stats.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `TorrentStats.file_progress` | Always serialized | `#[serde(skip_serializing_if = "Vec::is_empty")]` — omitted in list view (after clearing in api.rs) | **Changed** |
| `TorrentStats.added_at` | N/A | `Option<DateTime<Utc>>` | **Added** |
| `TorrentStats.total_fetched_bytes` | N/A | `u64` — cumulative bytes downloaded across sessions | **Added** |
| `TorrentStats.last_activity` | N/A | `Option<DateTime<Utc>>` — last time any data was transferred | **Added** |

---

## 5. HTTP API Changes

### 5.1 New Endpoints (`http_api/handlers/`)

| Method | Endpoint | Handler | Description |
|---|---|---|---|
| `POST` | `/torrents/create_task` | `h_create_torrent_task_enqueue` | Enqueue a torrent creation job |
| `GET` | `/torrents/create_tasks` | `h_create_torrent_task_list` | List all creation tasks with status |
| `DELETE` | `/torrents/create_tasks/{id}` | `h_create_torrent_task_cancel` | Cancel a creation task by ID |
| `GET` | `/torrents/{id}/extra_files` | `h_torrent_extra_files_list` | List extra files not in the torrent manifest |
| `POST` | `/torrents/{id}/delete_extra_files` | `h_torrent_extra_files_delete` | Delete specified extra files from disk |

### 5.2 Modified Endpoints

| Endpoint | Change |
|---|---|
| `POST /torrents/create` | Added `stream: bool` query param. When `stream: true`, returns NDJSON SSE stream with `{type: "progress", chunk, total}` and `{type: "success", id}` / `{type: "error", error}` events. `CreateTorrentOptions.name` changed from `Option<&str>` to `Option<String>`. |
| `GET /torrents` (list) | Strips `files`, `trackers`, and `file_progress` from response for polling performance. |

### 5.3 `http_api_types.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `TorrentAddQueryParams.skip_initial_check` | N/A | `Option<bool>` — maps to `AddTorrentOptions.skip_initial_check` | **Added** |
| `TorrentAddQueryParams.sync_extra_files` | N/A | `Option<bool>` — maps to `AddTorrentOptions.sync_extra_files` (per-torrent override) | **Added** |
| `AddTorrentRequest` struct | N/A | New: `url: Option<String>`, `data_base64: Option<String>`, flattened `TorrentAddQueryParams` | **Added** |

---

## 6. Peer Connection & Networking Changes

### 6.1 `listen.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| Default uTP link MTU | None (uses library default) | Defaults to **1400** if not set, to avoid `WSAEMSGSIZE` (error 10040) on PPPoE/VPN/VLAN networks with MTU < 1500 | **Added** |

---

## 7. Session Persistence Changes

### 7.1 `session_persistence/mod.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `SerializedTorrent.added_at` | N/A | `Option<DateTime<Utc>>` — persisted and restored | **Added** |
| `SerializedTorrent.total_fetched_bytes` | N/A | `u64` — persisted and restored (default 0) | **Added** |
| `SerializedTorrent.last_activity` | N/A | `Option<DateTime<Utc>>` — persisted and restored | **Added** |
| `into_add_torrent()` | — | Passes `added_at`, `total_fetched_bytes`, `last_activity` to `AddTorrentOptions` | **Changed** |

### 7.2 `session_persistence/json.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `store_torrent()` | Saves basic fields | Additionally saves `added_at`, `total_fetched_bytes`, `last_activity` from `ManagedTorrent` | **Changed** |

---

## 8. Desktop App Changes

### 8.1 `desktop/src-tauri/src/main.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| Portable mode | N/A | `get_config_path()` checks for `config.json` next to `.exe` first; falls back to `%APPDATA%` | **Added** |
| System tray | N/A | Full system tray: "Show", "Start at Login" (via `tauri_plugin_autostart`), "Quit"; click-to-show; close-to-tray | **Added** |
| Version command | `env!("CARGO_PKG_VERSION")` | `env!("RQBIT_VERSION")` — injected by `build.rs` | **Changed** |
| `torrent_create` command | N/A | Creates torrent with progress events (throttled to 2000ms), auto-adds to session | **Added** |
| `torrent_create_task_enqueue` command | N/A | Enqueue creation task via `TorrentCreationManager` | **Added** |
| `torrent_create_task_list` command | N/A | List all creation tasks | **Added** |
| `torrent_create_task_cancel` command | N/A | Cancel creation task via `AbortHandle` (aborts the spawned hashing task) | **Added** |
| `torrent_create_task_delete` command | N/A | Delete completed/cancelled task | **Added** |
| `torrent_list_extra_files` command | N/A | List extra files for a torrent (delegates to `api_list_extra_files`) | **Added** |
| `torrent_delete_extra_files` command | N/A | Delete specified extra files (delegates to `api_delete_extra_files`) | **Added** |
| `get_limits` / `set_limits` commands | N/A | Get/set rate limits at runtime; persists to config | **Added** |
| Basic Auth | `basic_auth: None` | Parses `config.http_api.basic_auth` as `"user:pass"` string | **Added** |
| Logging | `log_file: None` | Console-only logging at `info` level (no file output). Log lines are broadcast via Tauri events to the frontend. | **Unchanged** |
| Log broadcast | N/A | Subscribes to `line_broadcast` and emits `log_line` Tauri events | **Added** |
| SessionOptions propagation | — | Passes `skip_hash_check`, `kill_locking_processes`, `sync_extra_files`, `permissive_file_opening`, `enable_file_integrity_monitor`, `concurrent_init_limit` | **Added** |
| Config error handling | `if let Ok(config)` (silently ignores) | `match` with explicit warning on failure: `"failed reading config from..."` | **Changed** |
| Tauri plugins added | `tauri_plugin_shell` only | + `tauri_plugin_autostart`, `tauri_plugin_dialog`, `tauri_plugin_clipboard_manager` | **Added** |
| Window close behavior | Default quit | Prevents close; hides window to tray instead | **Changed** |

### 8.2 `desktop/src-tauri/src/config.rs`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `RqbitDesktopConfigFeatures` struct | N/A | New: `kill_locking_processes`, `sync_extra_files`, `permissive_file_opening`, `enable_file_integrity_monitor` (all `bool`, default false) | **Added** |
| `RqbitDesktopConfig.features` | N/A | `RqbitDesktopConfigFeatures` field | **Added** |
| `RqbitDesktopConfig.concurrent_init_limit` | N/A | `usize`, default 3 | **Added** |
| `RqbitDesktopConfigPersistence.skip_hash_check` | N/A | `bool`, default false | **Added** |
| `RqbitDesktopConfigHttpApi.basic_auth` | N/A | `Option<String>` | **Added** |

### 8.3 `desktop/src-tauri/build.rs` (NEW)

| Item | Description |
|---|---|
| Version injection | Reads version from `Cargo.toml`, sets `RQBIT_VERSION` env var at compile time |

### 8.4 `desktop/src-tauri/capabilities/default.json`

| Item | Upstream | Fork | Type |
|---|---|---|---|
| Permissions | Basic | Added: `shell:allow-open`, `autostart:allow-enable/disable/is-enabled`, `dialog:default`, `clipboard-manager:default` | **Added** |

### 8.5 Desktop Frontend (`desktop/src/`)

| File | Change | Type |
|---|---|---|
| `api.tsx` | Added `createTorrent()`, `createTorrentTask()`, `listCreateTorrentTasks()`, `cancelCreateTorrentTask()`, `deleteCreateTorrentTask()`, `listExtraFiles()`, `removeExtraFiles()` API methods; `getStreamLogsUrl()` returns `null` (desktop uses Tauri events instead) | **Changed/Added** |
| `configuration.tsx` | Added interfaces: `RqbitDesktopConfigFeatures` (`kill_locking_processes`, `sync_extra_files`, `permissive_file_opening`, `enable_file_integrity_monitor`); added `skip_hash_check`, `basic_auth`, `concurrent_init_limit` to config types | **Added** |
| `configure.tsx` | Added **Features tab** (kill locking, sync extra, permissive, file integrity monitor); added **Basic Auth** field to HTTP API tab; added **Skip hash check** and **Concurrent hash check limit** to Session tab; restructured from flat to tabbed layout | **Changed** |
| `rqbit-desktop.tsx` | Integrated Tauri events for `log_line` and `create_torrent_progress`; localhost security enforced to `http://127.0.0.1:3030` with strict CSP | **Changed** |

---

## 9. WebUI Changes

### 9.1 API Types (`api-types.ts`)

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `TorrentStats.added_at` | N/A | `string \| null` (ISO datetime) | **Added** |
| `TorrentStats.total_fetched_bytes` | N/A | `number` | **Added** |
| `TorrentStats.last_activity` | N/A | `string \| null` (ISO datetime) | **Added** |
| `TorrentDetails.trackers` | N/A | `string[] | null` | **Added** |
| `AddTorrentResponse.already_managed` | N/A | `boolean` | **Added** |
| `AddTorrentOptions.sync_extra_files` | N/A | `boolean | null` — per-torrent override for auto-delete extra files | **Added** |
| `CreateTorrentTask` | N/A | Interface: `id`, `status`, `source_path`, `processed_bytes`, `total_bytes`, `magnet_link`, `error`, `created_at` | **Added** |
| `RqbitAPI` interface | — | Added: `createTorrent()`, `createTorrentTask()`, `listCreateTorrentTasks()`, `cancelCreateTorrentTask()`, `deleteCreateTorrentTask()`, `listExtraFiles()`, `removeExtraFiles()` | **Added** |

### 9.2 Stores (`stores/`)

| File | Change | Type |
|---|---|---|
| `torrentStore.ts` | Added `torrentsEqual()` shallow equality check; prevents Zustand re-renders when data is identical; split into separate `setTorrents` / `setGlobalStats` actions | **Changed** |
| `statsStore.ts` | Added equality check for stats; adaptive polling support | **Changed** |

### 9.3 Components (`components/`)

| Component | Change | Type |
|---|---|---|
| `TorrentTable.tsx` | Stabilized `itemContent` callback via `useRef` to prevent Virtuoso re-renders; added `Recv`, `Remaining`, `Last Active` columns | **Changed** |
| `TorrentTableRow.tsx` | Added `Recv` (received bytes ≠ downloaded), `Remaining`, `Last Active` column data; "Seeding" status for 100% complete torrents | **Changed** |
| `TableHeader.tsx` | New column headers: `Recv`, `Rem`, `Last Active` | **Changed** |
| `CompactLayout.tsx` | Fixed `mousemove` listener leak that accumulated handlers on every render | **Fixed** |
| `FileListInput.tsx` | `useMemo` depends only on `torrentDetails` (not full stats); progress bars fetch data dynamically | **Changed** |
| `PiecesCanvas.tsx` | Deep bitmap equality check avoids redundant canvas redraws | **Changed** |
| `DetailPane.tsx` | Added tracker display; shows `Recv` / `Remaining` stats; integrated `CopyMagnetButton`; passes `torrentName` to `FilesTab` | **Changed** |
| `FilesTab.tsx` | Accepts `torrentName` prop for modal display | **Changed** |
| `FileSelectionModal.tsx` | Smart file selection: when all files are selected, `only_files` is omitted from the API call to reduce request size. Added "Auto-delete extra files" checkbox (`sync_extra_files`) with localStorage persistence (`rqbit_sync_extra_files` key), remembering user preference across sessions | **Changed** |
| `ConfigModal.tsx` | Added rate limits configuration UI | **Changed** |
| `CreateTorrentModal.tsx` | Full creation dialog with directory picker, tracker input, progress bar, success state with magnet link | **Added** |
| `CreateTorrentQueueModal.tsx` | Queue monitor: shows pending/processing/done/error/cancelled tasks with progress, cancel, and delete actions | **Added** |
| `CleanExtraFilesModal.tsx` | Manual extra file cleanup: lists files not in torrent manifest, select-all/individual toggles, delete with result summary (removed/failed counts) | **Added** |
| `CopyMagnetButton.tsx` | 3-tier clipboard: Tauri clipboard plugin → `navigator.clipboard` → textarea fallback; visual feedback | **Added** |

### 9.4 Polling Architecture (`rqbit-web.tsx`)

| Item | Upstream | Fork | Type |
|---|---|---|---|
| Polling loop | Multiple independent `setInterval` calls | Single `setTimeout`-based loop using `Promise.all` for torrents + stats | **Changed** |
| Adaptive intervals | Fixed | **2s** active, **5s** idle, **10s** tab-hidden (via `useDocumentVisibility` hook) | **Changed** |
| Stats in list | `with_stats` param via frontend | `torrents_list` command always uses `with_stats: true` directly in Rust (bypasses camelCase/snake_case serialization mismatch) | **Changed** |

### 9.5 HTTP API Client (`http-api.ts`)

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `createTorrent()` | N/A | `POST /torrents/create` with path body and query opts | **Added** |
| `createTorrentTask()` | N/A | `POST /torrents/create_task` | **Added** |
| `listCreateTorrentTasks()` | N/A | `GET /torrents/create_tasks` | **Added** |
| `cancelCreateTorrentTask()` | N/A | `DELETE /torrents/create_tasks/{id}` | **Added** |
| `deleteCreateTorrentTask()` | N/A | `DELETE /torrents/create_tasks/{id}` | **Added** |
| `listExtraFiles()` | N/A | `GET /torrents/{id}/extra_files` | **Added** |
| `removeExtraFiles()` | N/A | `POST /torrents/{id}/delete_extra_files` | **Added** |
| `uploadTorrent()` query params | N/A | Added `sync_extra_files=true` query parameter support | **Changed** |

---

## 10. Build & Tooling Changes

| Item | Upstream | Fork | Type |
|---|---|---|---|
| `Cargo.lock` | — | +473 lines (new dependencies: `chrono`, `walkdir`, `urlencoding`, `windows` crate for Restart Manager, Tauri plugins) | **Changed** |
| `librqbit/Cargo.toml` | — | Added: `chrono`, `walkdir`, `urlencoding`, `windows` (features: `Win32_System_RestartManager`, `Win32_Foundation`) | **Changed** |
| `desktop/src-tauri/Cargo.toml` | — | Added: `tauri-plugin-autostart`, `tauri-plugin-dialog`, `tauri-plugin-clipboard-manager`, `chrono` | **Changed** |
| `desktop/src-tauri/build.rs` | N/A | Reads version from Cargo.toml, sets `RQBIT_VERSION` env var | **Added** |
| `.gitignore` | — | Added entries for build artifacts | **Changed** |
| `webui/package.json` | — | Updated dependencies (+7 lines) | **Changed** |
| Zero-warning build | Not guaranteed | Codebase compiles with **0 warnings** and **0 errors** | **Changed** |

---

## 11. Known Pitfalls & Prevention Guidelines

### API Response Size
> **Rule**: Never return `files` or `trackers` in the torrent list endpoint.

The list API is polled every 2–5 seconds. With 40+ torrents (some with 61,000+ files), including these fields would generate multi-MB JSON payloads. The optimization in `api.rs` (`r.files = None; r.trackers = None; stats.file_progress.clear()`) is intentional.

### Polling Architecture
> **Rule**: Use a single `setTimeout` loop, never multiple `setInterval` calls.

Multiple intervals cause overlapping API calls. The combined loop in `rqbit-web.tsx` fetches both torrents and stats via `Promise.all`, then schedules the next iteration with `setTimeout`.

### Debug Logging
> **Rule**: Never use `remoteLog` or `invoke("frontend_log")` in production builds.

Each call triggers a Tauri IPC round-trip. The `remoteLogger.ts` helper has been deleted.

### Tauri Command Serialization
> **Rule**: Tauri commands receive snake_case; frontend sends camelCase.

For critical parameters like `with_stats`, hard-code the value in Rust rather than relying on frontend-passed options.

### Zustand Store Updates
> **Rule**: Always use equality checks in Zustand stores.

The `torrentsEqual` function performs shallow comparison to skip unnecessary React re-renders.

### Kill Locking Processes
> **Rule**: Treat `kill_locking_processes` as a destructive operation. Default is `false`.

The Win32 Restart Manager API (`RmShutdown`) will terminate **any** process holding a file lock — including other running applications. Only enable for environments where the download directory is dedicated to rqbit.

### Skip Hash Check
> **Rule**: Only enable `skip_hash_check` when restoring a known-good session.

Skipping hash verification means corrupted or modified files will be treated as complete. If a file was altered between sessions, rqbit will seed bad data. Always do a full check after abnormal shutdowns. When the File Integrity Monitor is also enabled, startup validation checks are performed (size + mtime) to catch modifications that occurred while the app was closed.

### File Integrity Monitor (FIM)
> **Rule**: Enable `enable_file_integrity_monitor` to protect peers from corrupted data during seeding.

When enabled, FIM captures a baseline of file metadata (mtime + size) when a torrent enters the seeding state and periodically rechecks. If an external process modifies or deletes a file, the torrent is auto-paused via `on_fatal_error()`. The polling interval adapts to file count (60s for ≤1K files, up to 300s for >50K files). The baseline is **automatically refreshed** when download completes (`chunks.is_finished()`), so download writes don't trigger false positives. Note: `sync_extra_files` deletions do **not** affect the baseline because `file_metadata()` only tracks torrent files, not extra files. FIM is **opt-in** (default: `false`) — it adds periodic `stat()` syscalls per file which may increase disk I/O on very large torrents. FIM also enables startup integrity validation when `skip_hash_check` is on.

### Permissive File Opening (`FILE_SHARE_DELETE`)
> **Rule**: `permissive_file_opening` completely disables the LRU file handle cache.

When enabled, files are opened with `FILE_SHARE_READ | WRITE | DELETE` and the `file_cache` is set to `None` (not just capacity 0 — the cache is entirely absent). This ensures no file handles are held open, which on Windows prevents parent-directory locking that would block rename/move/delete operations. The tradeoff is increased syscall overhead since every piece read/write re-opens the file handle.

### LRU Cache Mode Tracking
> **Rule**: The LRU cache tracks whether each handle is writable. Never return a read-only handle for a write operation.

The LRU cache stores `(Arc<File>, bool)` tuples where the `bool` indicates whether the handle was opened with write access. During `initial_check()`, files are opened read-only for hash verification and cached. When download starts and `pwrite_all()` requests a writable handle, `get_or_open()` detects the mode mismatch, evicts the stale read-only handle, and re-opens the file with write access. Without this, `seek_write()` on a read-only handle causes `ERROR_ACCESS_DENIED` (OS error 5). As defense-in-depth, `pwrite_all()` includes a **handle-eviction retry**: if a write fails with Access Denied, the handle is evicted and re-opened before retrying. `pwrite_all_vectored()` performs handle eviction but **cannot retry** the write (the `IoSlice` buffers are consumed on the first attempt) — instead it ensures the next write from the torrent engine uses a fresh handle. Additionally, `pwrite_all()` on Windows retries `seek_write()` up to 3 times with backoff for transient OS-level errors (antivirus scanning, NTFS journal updates).

### File Modification Timestamps
> **Rule**: File modification timestamps are preserved on restart.

The `ensure_file_length()` call during torrent initialization checks `std::fs::metadata().len()` before calling `File::set_len()`. On Windows, `set_len()` invokes Win32 `SetEndOfFile` which **always updates the modification timestamp**, even when the file size hasn't changed. By skipping `set_len()` for correctly-sized files (100% downloaded torrents), original modification timestamps are preserved across rqbit restarts.

---

## 12. Build Instructions

### Prerequisites
*   **Visual Studio 2022 Build Tools** (C++ Workload)
*   **Node.js** (v18+) & **npm**
*   **Rust (Cargo)** (Stable)

### Building for Release

```powershell
# Full build (frontend + backend + installer)
cd desktop
npm install
npm run tauri build

# Or manual two-step build:
# 1. Build Frontend
npm run build
cd ..

# 2. Build Backend (Enable custom-protocol for Tauri v2 assets)
cargo build --release -p rqbit-desktop --features rqbit-desktop/custom-protocol
```

### Output
*   **Executable**: `target/release/rqbit-desktop.exe`
*   **MSI Installer**: `target/release/bundle/msi/rqbit_*.msi`
*   **NSIS Installer**: `target/release/bundle/nsis/rqbit_*-setup.exe`

### CI (this fork)
Linux jobs use **Ubicloud** (`runs-on: ubicloud`). Windows jobs use GitHub-hosted `windows-latest`. Details in `README.feature_strategy.md`.

---

## 13. Credits
*   **Original Author**: [Igor Katson](https://github.com/ikatson)
*   **Upstream Repo**: [ikatson/rqbit](https://github.com/ikatson/rqbit)

---
*This fork is maintained for high-performance Windows environments.*

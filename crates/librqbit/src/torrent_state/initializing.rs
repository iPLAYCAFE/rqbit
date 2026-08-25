use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use anyhow::Context;

use rand::{RngExt, seq::IteratorRandom};
use size_format::SizeFormatterBinary as SF;
use tracing::{info, trace, warn};

use crate::{
    api::TorrentIdOrHash,
    bitv::BitV,
    bitv_factory::BitVFactory,
    chunk_tracker::{ChunkTracker, compute_selected_pieces},
    file_ops::FileOps,
    type_aliases::{BF, FileStorage},
};

const MAX_FASTRESUME_CHECKS: usize = 64;

use super::{ManagedTorrentShared, TorrentMetadata, paused::TorrentStatePaused};

pub struct TorrentStateInitializing {
    pub(crate) files: FileStorage,
    pub(crate) shared: Arc<ManagedTorrentShared>,
    pub(crate) metadata: Arc<TorrentMetadata>,
    pub(crate) only_files: Option<Vec<usize>>,
    pub(crate) checked_bytes: AtomicU64,
    pause_requested: AtomicBool,
    check_running: AtomicBool,
    previously_errored: bool,
    skip_check: bool,
}

impl TorrentStateInitializing {
    pub fn new(
        shared: Arc<ManagedTorrentShared>,
        metadata: Arc<TorrentMetadata>,
        only_files: Option<Vec<usize>>,
        files: FileStorage,
        previously_errored: bool,
        skip_check: bool,
    ) -> Self {
        Self {
            shared,
            metadata,
            only_files,
            files,
            checked_bytes: AtomicU64::new(0),
            pause_requested: AtomicBool::new(false),
            check_running: AtomicBool::new(false),
            previously_errored,
            skip_check,
        }
    }

    pub fn get_checked_bytes(&self) -> u64 {
        self.checked_bytes
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn request_pause(&self) {
        self.pause_requested.store(true, Ordering::Relaxed);
    }

    pub(crate) fn clear_pause_request(&self) {
        self.pause_requested.store(false, Ordering::Relaxed);
    }

    pub(crate) fn is_pause_requested(&self) -> bool {
        self.pause_requested.load(Ordering::Relaxed)
    }

    pub(crate) fn try_start_check(&self) -> bool {
        self.check_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn finish_check(&self) {
        self.check_running.store(false, Ordering::Release);
    }

    async fn validate_fastresume(
        &self,
        bitv_factory: &dyn BitVFactory,
        have_pieces: Option<Box<dyn BitV>>,
    ) -> Option<Box<dyn BitV>> {
        let hp = have_pieces?;
        let actual = hp.as_bytes().len();
        let expected = self.metadata.lengths().piece_bitfield_bytes();
        if actual != expected {
            warn!(
                actual,
                expected,
                "the bitfield loaded isn't of correct length, ignoring it, will do full check"
            );
            return None;
        }

        let is_broken = self
            .shared
            .spawner
            .block_in_place_with_semaphore(|| {
                let fo = crate::file_ops::FileOps::new(
                    &self.metadata.info,
                    &self.files,
                    &self.metadata.file_infos,
                );

                let mut to_validate = BF::from_boxed_slice(
                    vec![0u8; self.metadata.lengths().piece_bitfield_bytes()].into_boxed_slice(),
                );
                let mut queue = hp.as_slice().to_owned();

                // Validate at least one piece from each file, if we claim we have it.
                for fi in self.metadata.file_infos.iter() {
                    let prange = fi.piece_range_usize();
                    let offset = prange.start;
                    for piece_id in hp
                        .as_slice()
                        .get(fi.piece_range_usize())
                        .into_iter()
                        .flat_map(|s| s.iter_ones())
                        .map(|pid| pid + offset)
                        .take(1)
                    {
                        to_validate.set(piece_id, true);
                        queue.set(piece_id, false);
                    }
                }

                // For all the remaining pieces we claim we have, validate them with decreasing probability.
                let queue = queue
                    .iter_ones()
                    .sample(&mut rand::rng(), MAX_FASTRESUME_CHECKS);

                for (tmp_id, piece_id) in queue.into_iter().enumerate() {
                    let denom: u32 = (tmp_id + 1).min(50).try_into().unwrap();
                    if rand::rng().random_ratio(1, denom) {
                        to_validate.set(piece_id, true);
                    }
                }

                let to_validate_count = to_validate.count_ones();
                for (id, piece_id) in to_validate
                    .iter_ones()
                    .filter_map(|id| {
                        self.metadata
                            .lengths()
                            .validate_piece_index(id.try_into().ok()?)
                    })
                    .enumerate()
                {
                    if fo.check_piece(piece_id).is_err() {
                        return true;
                    }

                    #[allow(clippy::cast_possible_truncation)]
                    let progress = (self.metadata.lengths().total_length() as f64
                        / to_validate_count as f64
                        * (id + 1) as f64) as u64;
                    let progress = progress.min(self.metadata.lengths().total_length());
                    self.checked_bytes.store(progress, Ordering::Relaxed);
                }

                false
            })
            .await;

        if is_broken {
            warn!(
                id = ?self.shared.id,
                info_hash = ?self.shared.info_hash,
                "data corrupted, ignoring fastresume data"
            );
            if let Err(e) = bitv_factory.clear(self.shared.id.into()).await {
                warn!(id=?self.shared.id, info_hash = ?self.shared.info_hash, "error clearing bitfield: {e:#}");
            }
            self.checked_bytes.store(0, Ordering::Relaxed);
            return None;
        }

        Some(hp)
    }

    pub async fn check(&self) -> anyhow::Result<TorrentStatePaused> {
        let id: TorrentIdOrHash = self.shared.info_hash.into();
        let bitv_factory = self
            .shared
            .session
            .upgrade()
            .context("session is dead")?
            .bitv_factory
            .clone();
        let have_pieces = if self.previously_errored {
            if let Err(e) = bitv_factory.clear(id).await {
                warn!(id=?self.shared.id, info_hash = ?self.shared.info_hash, error=?e, "error clearing bitfield");
            }
            None
        } else {
            bitv_factory
                .load(id)
                .await
                .context("error loading have_pieces")?
        };

        if self.skip_check {
            // Startup integrity validation: even with skip_check, detect files modified
            // while rqbit was shut down. This prevents seeding corrupted data.
            // Only runs when enable_file_integrity_monitor is enabled.
            if self.shared.options.enable_file_integrity_monitor {
                let mut integrity_ok = true;

                // 1. Quick size check: compare actual file sizes against .torrent metadata
                let current_metadata = self.files.file_metadata().unwrap_or_default();
                for (idx, fi) in self.metadata.file_infos.iter().enumerate() {
                    if fi.attrs.padding {
                        continue;
                    }
                    if let Some(Some((_mtime, size))) = current_metadata.get(idx)
                        && *size != fi.len
                    {
                        warn!(
                            file = %fi.relative_filename.display(),
                            expected_size = fi.len,
                            actual_size = size,
                            "File size mismatch detected at startup — forcing full hash check"
                        );
                        integrity_ok = false;
                        break;
                    }
                }

                // 2. mtime check: compare file mtimes against .bitv file mtime
                if integrity_ok {
                    let bitv_mtime = bitv_factory.get_mtime(id).await.unwrap_or(None);
                    if let Some(bitv_mtime) = bitv_mtime {
                        for (idx, fi) in self.metadata.file_infos.iter().enumerate() {
                            if fi.attrs.padding {
                                continue;
                            }
                            if let Some(Some((file_mtime, _size))) = current_metadata.get(idx)
                                && *file_mtime > bitv_mtime
                            {
                                warn!(
                                    file = %fi.relative_filename.display(),
                                    "File modified after last session — forcing full hash check"
                                );
                                integrity_ok = false;
                                break;
                            }
                        }
                    }
                }

                // If integrity check failed, pause the torrent with an error
                if !integrity_ok {
                    anyhow::bail!(
                        "Files were modified while rqbit was shut down. \
                         Torrent paused to prevent serving corrupted data. \
                         Use Force Recheck to verify and resume."
                    );
                }
            }

            // Integrity OK — proceed with skip check as normal
            use bitvec::vec::BitVec;
            let num_bytes = self.metadata.lengths().piece_bitfield_bytes();
            let mut bv = BitVec::<u8, bitvec::order::Msb0>::from_vec(vec![0xff; num_bytes]);
            let _total_pieces = self.metadata.lengths().total_pieces() as usize;
            let expected_bits = num_bytes * 8;
            if bv.len() != expected_bits {
                bv.resize(expected_bits, true);
            }

            let bv = bv.into_boxed_bitslice();

            info!("Startup integrity check passed — skipping full hash check");

            self.checked_bytes
                .store(self.metadata.lengths().total_length(), Ordering::Relaxed);

            bitv_factory
                .store_initial_check(id, bv.clone())
                .await
                .context("error storing skipped check bitfield")?;

            return self.finalize_check(Box::new(bv)).await;
        }

        let have_pieces = self.validate_fastresume(&*bitv_factory, have_pieces).await;

        let have_pieces = match have_pieces {
            Some(h) => h,
            None => {
                info!("Doing initial checksum validation, this might take a while...");
                let have_pieces = self
                    .shared
                    .spawner
                    .block_in_place_with_semaphore(|| {
                        FileOps::new(&self.metadata.info, &self.files, &self.metadata.file_infos)
                            .initial_check(&self.checked_bytes, &self.pause_requested)
                    })
                    .await?;
                bitv_factory
                    .store_initial_check(id, have_pieces)
                    .await
                    .context("error storing initial check bitfield")?
            }
        };
        self.finalize_check(have_pieces).await
    }

    async fn finalize_check(
        &self,
        have_pieces: Box<dyn crate::bitv::BitV>,
    ) -> anyhow::Result<TorrentStatePaused> {
        let selected_pieces = compute_selected_pieces(
            self.metadata.lengths(),
            |idx| {
                self.only_files
                    .as_ref()
                    .map(|o| o.contains(&idx))
                    .unwrap_or(true)
            },
            &self.metadata.file_infos,
        );

        let chunk_tracker = ChunkTracker::new(
            have_pieces.into_dyn(),
            selected_pieces,
            *self.metadata.lengths(),
            &self.metadata.file_infos,
        )
        .context("error creating chunk tracker")?;

        let hns = chunk_tracker.get_hns();

        // Sync extra files only for newly added torrents (not restored from session),
        // when the torrent is already fully complete after hash check.
        if self.shared.options.sync_extra_files
            && !self.shared.options.is_restoring
            && hns.finished()
        {
            use crate::sync_utils::remove_extra_files;
            info!("Syncing extra files...");
            if let Err(e) = remove_extra_files(
                self.metadata.info.info(),
                &self.shared.options.output_folder,
            ) {
                warn!("Error removing extra files: {:#}", e);
            }
        }

        info!(
            torrent=?self.shared.id,
            "Check results: have {}, needed {}, total selected {}",
            SF::new(hns.have_bytes),
            SF::new(hns.needed_bytes),
            SF::new(hns.selected_bytes)
        );

        // Ensure file lengths are correct, and reopen read-only.
        self.shared
            .spawner
            .block_in_place_with_semaphore(|| {
                for (idx, fi) in self.metadata.file_infos.iter().enumerate() {
                    if self
                        .only_files
                        .as_ref()
                        .map(|v| v.contains(&idx))
                        .unwrap_or(true)
                    {
                        let now = Instant::now();
                        if fi.attrs.padding {
                            continue;
                        }
                        if let Err(err) = self.files.ensure_file_length(idx, fi.len) {
                            let is_permission_denied = err
                                .root_cause()
                                .downcast_ref::<std::io::Error>()
                                .map(|io_err| io_err.kind() == std::io::ErrorKind::PermissionDenied)
                                .unwrap_or(false);

                            if is_permission_denied {
                                tracing::debug!(
                                    id=?self.shared.id, info_hash = ?self.shared.info_hash,
                                    "Error setting length for file {:?} to {} (read-only?): {:#?}",
                                    fi.relative_filename, fi.len, err
                                );
                            } else {
                                warn!(
                                    id=?self.shared.id, info_hash = ?self.shared.info_hash,
                                    "Error setting length for file {:?} to {}: {:#?}",
                                    fi.relative_filename, fi.len, err
                                );
                            }
                        } else {
                            trace!(
                                "Set length for file {:?} to {} in {:?}",
                                fi.relative_filename,
                                SF::new(fi.len),
                                now.elapsed()
                            );
                        }
                    }
                }
                Ok::<_, anyhow::Error>(())
            })
            .await?;

        let paused = TorrentStatePaused {
            shared: self.shared.clone(),
            metadata: self.metadata.clone(),
            files: self.files.take()?,
            chunk_tracker,
            streams: Arc::new(Default::default()),
        };
        Ok(paused)
    }
}

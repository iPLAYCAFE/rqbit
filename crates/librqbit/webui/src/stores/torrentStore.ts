import { create } from "zustand";
import { TorrentDetails, TorrentListItem } from "../api-types";

// Specialized comparison for TorrentListItem to avoid expensive deep equality checks
function torrentsEqual(a: TorrentListItem, b: TorrentListItem): boolean {
  if (a === b) return true;
  if (a.id !== b.id) return false;
  if (a.info_hash !== b.info_hash) return false;
  if (a.name !== b.name) return false;
  if (a.output_folder !== b.output_folder) return false;
  if (a.total_pieces !== b.total_pieces) return false;

  // Stats comparison
  const s1 = a.stats;
  const s2 = b.stats;

  if (s1 === s2) return true;
  if (!s1 || !s2) return false;

  if (s1.state !== s2.state) return false;
  if (s1.error !== s2.error) return false;
  if (s1.progress_bytes !== s2.progress_bytes) return false;
  if (s1.finished !== s2.finished) return false;
  if (s1.total_bytes !== s2.total_bytes) return false;
  if (s1.total_fetched_bytes !== s2.total_fetched_bytes) return false;
  if (s1.added_at !== s2.added_at) return false;
  if (s1.last_activity !== s2.last_activity) return false;

  // Live stats comparison
  const l1 = s1.live;
  const l2 = s2.live;

  if (l1 === l2) return true;
  if (!l1 || !l2) return false;

  // Compare high-frequency changing fields
  // Compare high-frequency changing fields
  // Use human_readable to avoid re-renders on floating point noise
  if (l1.download_speed.human_readable !== l2.download_speed.human_readable) return false;
  if (l1.upload_speed.human_readable !== l2.upload_speed.human_readable) return false;
  
  if (l1.snapshot.uploaded_bytes !== l2.snapshot.uploaded_bytes) return false;
  if (l1.snapshot.fetched_bytes !== l2.snapshot.fetched_bytes) return false;
  
  // Compare peer stats
  const p1 = l1.snapshot.peer_stats;
  const p2 = l2.snapshot.peer_stats;
  // CompactView only displays live/seen
  if (p1.live !== p2.live || p1.seen !== p2.seen) return false;

  // Compare ETA
  const t1 = l1.time_remaining;
  const t2 = l2.time_remaining;
  if (t1 !== t2) { // check ref
      if (!t1 || !t2) return false;
      // assuming time_remaining structure is simple
      if (t1.human_readable !== t2.human_readable) return false;
  }

  return true;
}

export interface TorrentStore {
  torrents: Array<TorrentListItem> | null;
  setTorrents: (torrents: Array<TorrentListItem>) => void;

  torrentsInitiallyLoading: boolean;
  torrentsLoading: boolean;
  setTorrentsLoading: (loading: boolean) => void;

  refreshTorrents: () => void;
  setRefreshTorrents: (callback: () => void) => void;

  // TorrentDetails cache (keyed by torrent id)
  detailsCache: Map<number, TorrentDetails>;
  getDetails: (id: number) => TorrentDetails | null;
  setDetails: (id: number, details: TorrentDetails) => void;
}

export const useTorrentStore = create<TorrentStore>((set, get) => ({
  torrents: null,
  torrentsLoading: false,
  torrentsInitiallyLoading: false,
  setTorrentsLoading: (loading: boolean) =>
    set((prev) => {
      if (prev.torrents == null) {
        return { torrentsInitiallyLoading: loading, torrentsLoading: loading };
      }
      return { torrentsInitiallyLoading: false, torrentsLoading: loading };
    }),
  setTorrents: (newTorrents) =>
    set((prev) => {
      if (!prev.torrents) {
        return { torrents: newTorrents };
      }

      // Build map of current torrents for O(1) lookup
      const currentMap = new Map(prev.torrents.map((t) => [t.id, t]));

      // Reuse old reference if torrent unchanged
      const mergedTorrents = newTorrents.map((newTorrent) => {
        const current = currentMap.get(newTorrent.id);
        if (current && torrentsEqual(current, newTorrent)) {
          return current; // Keep old reference
        }
        return newTorrent;
      });

      // Check if array itself changed
      const arrayChanged =
        mergedTorrents.length !== prev.torrents.length ||
        mergedTorrents.some((t, i) => t !== prev.torrents![i]);

      return arrayChanged ? { torrents: mergedTorrents } : {};
    }),
  refreshTorrents: () => {},
  setRefreshTorrents: (callback) => set({ refreshTorrents: callback }),

  // TorrentDetails cache
  detailsCache: new Map(),
  getDetails: (id) => get().detailsCache.get(id) ?? null,
  setDetails: (id, details) =>
    set((prev) => ({
      detailsCache: new Map(prev.detailsCache).set(id, details),
    })),
}));

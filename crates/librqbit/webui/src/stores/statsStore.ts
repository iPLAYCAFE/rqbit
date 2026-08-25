import { create } from "zustand";

import { SessionStats } from "../api-types";

export interface StatsStore {
  stats: SessionStats;
  setStats: (stats: SessionStats) => void;
}

export const useStatsStore = create<StatsStore>((set) => ({
  stats: {
    counters: {
      fetched_bytes: 0,
      uploaded_bytes: 0,
      blocked_incoming: 0,
      blocked_outgoing: 0,
    },
    peers: {
      connecting: 0,
      dead: 0,
      live: 0,
      not_needed: 0,
      queued: 0,
      seen: 0,
    },
    download_speed: { human_readable: "N/A", mbps: 0 },
    upload_speed: { human_readable: "N/A", mbps: 0 },
    uptime_seconds: 0,
    connections: {
      tcp: {
        v4: {
          attempts: 0,
          successes: 0,
          errors: 0,
        },
        v6: {
          attempts: 0,
          successes: 0,
          errors: 0,
        },
      },
      utp: {
        v4: {
          attempts: 0,
          successes: 0,
          errors: 0,
        },
        v6: {
          attempts: 0,
          successes: 0,
          errors: 0,
        },
      },
      socks: {
        v4: {
          attempts: 0,
          successes: 0,
          errors: 0,
        },
        v6: {
          attempts: 0,
          successes: 0,
          errors: 0,
        },
      },
    },
  },
  setStats: (newStats) => {
    const prev = useStatsStore.getState().stats;
    // Only update if visible Footer fields actually changed
    if (
      prev.download_speed.human_readable === newStats.download_speed.human_readable &&
      prev.upload_speed.human_readable === newStats.upload_speed.human_readable &&
      prev.counters.fetched_bytes === newStats.counters.fetched_bytes &&
      prev.counters.uploaded_bytes === newStats.counters.uploaded_bytes &&
      prev.uptime_seconds === newStats.uptime_seconds
    ) {
      return; // No visible change, skip re-render
    }
    set({ stats: newStats });
  },
}));

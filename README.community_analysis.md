# Community Analysis & Production Impact — Feb 19, 2026

## Our PRs — Current Status

| PR | Title | Status | Latest Activity |
|---|---|---|---|
| [#555](https://github.com/ikatson/rqbit/pull/555) | 414 URI Too Long | ✅ **Merged** | — |
| [#556](https://github.com/ikatson/rqbit/pull/556) | LRU file handle cache | ⏳ Awaiting re-review | All feedback addressed + **mode tracking added** (`(Arc<File>, bool)`, eviction on mode mismatch, handle-retry). Rebased onto `f9b4aee8`. |
| [#557](https://github.com/ikatson/rqbit/pull/557) | Peer connection pruning | ⏳ Awaiting re-review | All Feb 14 feedback addressed: race condition fixed, `sort_unstable_by_key`, combined Dead+Queued phase, **4 unit tests added**. Force-pushed. |
| [#559](https://github.com/ikatson/rqbit/pull/559) | Sync extra files | ⏳ Awaiting re-review | Added `GET /torrents/{id}/extra_files` endpoint with full call chain (HTTP → Api → ManagedTorrent → TorrentStorage). Force-pushed. |
| [#560](https://github.com/ikatson/rqbit/pull/560) | Torrent creation | ⏳ Awaiting re-review | `watch::Sender` moved to `CreateTorrentOptions.on_progress`, CancellationToken removed (drop-based), return simplified to `CreateTorrentResult`. Force-pushed. |
| [#563](https://github.com/ikatson/rqbit/pull/563) | Portable mode | ⏳ Pending review | — |
| [#564](https://github.com/ikatson/rqbit/pull/564) | System tray + autostart | ⏳ Pending review | — |
| [#565](https://github.com/ikatson/rqbit/pull/565) | FILE_SHARE_DELETE (Windows) | ⏳ Pending review | — |
| [#566](https://github.com/ikatson/rqbit/pull/566) | ErrorBoundary (WebUI) | ❌ **Closed** | Closed by us — no concrete crash scenario to justify. |
| [#567](https://github.com/ikatson/rqbit/pull/567) | Skip redundant set_len | ⏳ Pending review | Replaces closed #558. No feedback yet. |
| [#570](https://github.com/ikatson/rqbit/pull/570) | Read-only fallback | ⏳ Awaiting re-review | Submitted Feb 15. Refs #136, #509. Supersedes stale #489, #373. **Coordination note added** for #556 mode tracking. Rebased onto `f9b4aee8`. |
| [#571](https://github.com/ikatson/rqbit/pull/571) | File integrity check | ⏳ Pending review | Submitted Feb 15. Refs #510, #509. |

**Summary:** 1 merged · 6 awaiting re-review (#556, #557, #559, #560, #570, #571) · 4 pending first review (#563–#565, #567) · 1 closed (#566).

---

## ✅ Resolved: LRU Cache Mode Tracking Bug

**Discovered Feb 19** during production testing. **Fixed and pushed to PR #556 on Feb 19.**

**Root cause:** `get_or_open()` returned cached file handles without checking whether the handle's access mode (read-only vs. writable) matched the requested mode. Files opened read-only during `initial_check()` were reused for `pwrite_all()`, causing `Access Denied (OS error 5)` on Windows.

**Fix applied (in fork + PR #556):**
1. Cache type: `LruCache<usize, (Arc<File>, bool)>` — tracks `is_writable`
2. `get_or_open()` evicts stale read-only handles when write is requested
3. `pwrite_all()`/`pwrite_all_vectored()` include handle-eviction retry on Access Denied
4. `opened_file.rs`: `seek_write()` retries 3x with backoff for transient OS errors (fork-only)

**PR #570** description updated with coordination note — if both merged, read-only handles enter LRU cache but #556's mode tracking correctly evicts them.

---

## Fork Features — Upstream Submission Status

### ✅ Merged

| Feature | PR |
|---|---|
| **414 URI Too Long** | #555 |

### ⏳ Awaiting Re-review (Updated Feb 19)

| Feature | PR | What Was Updated |
|---|---|---|
| **LRU file handle cache** | #556 | `(Arc<File>, bool)` mode tracking + eviction on mode mismatch + handle-retry. `cargo fmt` fixed. Rebased onto `f9b4aee8`. |
| **Read-only fallback** | #570 | Coordination note for #556. Rebased onto `f9b4aee8`. |

### ⏳ Awaiting Re-review (Feedback Addressed)

| Feature | PR | Issues | What Was Fixed |
|---|---|---|---|
| **Peer connection pruning** | #557 | #311 | Race condition fix, `sort_unstable_by_key`, 4 unit tests |
| **Sync extra files** | #559 | #335 | Added `GET /torrents/{id}/extra_files` API endpoint |
| **Torrent creation** | #560 | #333, #524 | `watch::Sender` in options, drop-based cancellation, `CreateTorrentResult` |

### ⏳ Pending First Review

| Feature | PR | Issues |
|---|---|---|
| **Portable mode** | #563 | #465 |
| **System tray + autostart** | #564 | #440 |
| **FILE_SHARE_DELETE** | #565 | #369, #120, #192 |
| **Skip redundant set_len** | #567 | #520 (mtime) |
| **File integrity check** | #571 | #510, #509 |

### ❌ Closed

| Feature | PR | Reason |
|---|---|---|
| **ErrorBoundary** | #566 | No concrete crash scenario — closed to focus on PRs solving real problems |

### 🟡 Possible Future PRs

| Feature | Pro | Con |
|---|---|---|
| `choose_piece_length` | Better torrent creation for large content | ikatson may prefer default |
| CopyMagnetButton | Clear UX improvement | Needs non-Tauri impl for WebUI |
| `seek_write` retry (Windows) | Defense-in-depth for transient OS errors | May be too Windows-specific |

### 🔴 Fork-Only (Do Not Submit)

| Feature | Reason |
|---|---|
| `kill_locking_processes` | Uses `taskkill /F` — too aggressive |
| `skip_hash_check` | Risk of data corruption |
| `added_at` / `last_activity` / `total_fetched_bytes` | Fork metadata — upstream may design differently |

---

## Community PRs (Non-iPLAYCAFE)

| PR | Author | Title | Status | Relevance |
|---|---|---|---|---|
| [#568](https://github.com/ikatson/rqbit/pull/568) | meatsquirk | BEP52: Merkle tree | ⚠️ Restructure requested | 🎯 Strategic |
| [#569](https://github.com/ikatson/rqbit/pull/569) | b-runz | IPv4 + portable atomic | Draft | ⚪ Niche |
| [#554](https://github.com/ikatson/rqbit/pull/554) | meatsquirk | BEP52 Phase 2 hybrid | Open | 🎯 Strategic |
| [#540](https://github.com/ikatson/rqbit/pull/540) | ivoviz | Tracker announce lifecycle | Open (15 comments) | ⚠️ High |
| [#527](https://github.com/ikatson/rqbit/pull/527) | costalfy | Document CLI options | Open | ⚪ Docs |
| [#512](https://github.com/ikatson/rqbit/pull/512) | bbb651 | Workspace dependencies | Draft | ⚪ Infra |
| [#510](https://github.com/ikatson/rqbit/pull/510) | fmckeogh | Verify on-disk data | Changes requested | Our #571 addresses same problem |
| [#506](https://github.com/ikatson/rqbit/pull/506) | fmckeogh | Pause/resume all torrents | Open | ⚪ UX |
| [#505](https://github.com/ikatson/rqbit/pull/505) | Stebalien | Systemd socket activation | ✅ **Merged** | ⚪ Linux |
| [#489](https://github.com/ikatson/rqbit/pull/489) | milahu | Seed completed read-only | Draft (stale) | Our #570 supersedes |
| [#373](https://github.com/ikatson/rqbit/pull/373) | Joaqim | Read-only seeding | Open (stale) | Our #570 supersedes |
| [#245](https://github.com/ikatson/rqbit/pull/245) | scottopell | JSON structured logs | Open | ⚪ Ops |

## Open Issues

| # | Title | Impact | Our Action |
|---|---|---|---|
| [#546](https://github.com/ikatson/rqbit/issues/546) | BEP 52 design | 🎯 Strategic | Tracking (meatsquirk) |
| [#539](https://github.com/ikatson/rqbit/issues/539) | Announce timing | ⚠️ | ivoviz PR #540 |
| [#537](https://github.com/ikatson/rqbit/issues/537) | Dual-stack announce | ⚪ | — |
| [#525](https://github.com/ikatson/rqbit/issues/525) | FD exhaustion | ⚠️ | → Our #556 |
| [#514](https://github.com/ikatson/rqbit/issues/514) | Release librqbit v9 | ⚪ | — |
| [#510](https://github.com/ikatson/rqbit/issues/510) | Verify on-disk data | ⚠️ | → Our #571 |
| [#509](https://github.com/ikatson/rqbit/issues/509) | Seed pre-downloaded torrent | ⚠️ | → Our #570 |
| [#507](https://github.com/ikatson/rqbit/issues/507) | Wrong tracker port | ⚠️ `planned` | — |
| [#504](https://github.com/ikatson/rqbit/issues/504) | File exists (os error 17) | ⚪ | — |
| [#561](https://github.com/ikatson/rqbit/issues/561) | Checking pause after disconnect | ⚪ | — |
| [#553](https://github.com/ikatson/rqbit/issues/553) | Radical client restart | ⚪ | — |
| [#551](https://github.com/ikatson/rqbit/issues/551) | Import from other clients | ⚪ | — |
| [#550](https://github.com/ikatson/rqbit/issues/550) | QoS/niceness/priorities | ⚪ | — |
| [#136](https://github.com/ikatson/rqbit/issues/136) | Read-only seeding | ⚠️ | → Our #570 |

---

## AI_POLICY.md

Created Feb 8: AI disclosure required, no low-effort AI PRs, validation burden on contributor.
**Key enforcement (Feb 14):** ikatson on #557: *"as this is all AI generated, it needs unit tests"* — we added 4 tests and passed.

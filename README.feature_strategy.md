# Feature & Contribution Strategy — Feb 19, 2026

## Guiding Principles

1. **Production first** — Our fork must never break.
2. **Follow upstream** — Align with maintainer's vision when submitting PRs.
3. **Contribute back** — Small, reviewable PRs (≤500 LoC). One feature each.
4. **No dead code** — Every PR must have callers. No unused methods.

---

## ikatson's Technical Preferences

| Preference | Implication | Source |
|---|---|---|
| Use existing crates | `lru`/`clru`, not custom implementations | #556 |
| Idiomatic tokio | `watch` for progress, future dropping for cancellation | #560 |
| Don't hold locks during I/O | Two-phase locking pattern | #556 |
| Minimal API surface | `&str` over `String` when possible | #560 |
| Storage subsystem abstraction | Features through `TorrentStorage` trait | #559 |
| UI-driven destructive ops | Confirmation dialog, not automatic | #559 |
| One feature per PR | No mixing unrelated changes | All |
| AI disclosure required | Honest and brief | AI_POLICY.md |
| PRs ≤500 LoC | Incremental, reviewable | All |
| Don't prune Live peers | Pruning targets Dead/Queued/NotNeeded only | #557 |
| AI code needs unit tests | *"as this is all AI generated, it needs unit tests"* | #557 |
| Progress via options, not return | `watch::Sender` in options so caller can monitor before completion | #560 |
| No unused/dead code in PRs | Every method must have a caller | #559, #568 |
| Top-down, not bottom-up | Start from callers, add primitives as needed | #568 |

---

## PR Status Overview

### ✅ Mode Tracking Fix Applied (Feb 19)

| PR | Title | What Was Updated |
|---|---|---|
| **#556** | LRU file handle cache | Cache stores `(Arc<File>, bool)` with mode tracking. `get_or_open()` evicts read-only handles when write requested. Handle-eviction retry in `pwrite_all()`. `cargo fmt` fixed. Rebased onto `f9b4aee8`. |
| **#570** | Read-only fallback | PR description updated with #556 coordination note. Rebased onto `f9b4aee8`. Both PRs fully compatible. |

> [!NOTE]
> **Resolved**: The latent bug in `get_or_open()` (returning read-only cached handles for write operations) has been fixed in both the fork and PR #556. PR #570 has a coordination note explaining compatibility.

### ⏳ Awaiting Re-review (Feedback Addressed)

| PR | Title | What Was Fixed |
|---|---|---|
| **#557** | Peer connection pruning | Race condition fixed, `sort_unstable_by_key`, combined Dead+Queued, **4 unit tests added** |
| **#559** | Sync extra files | Added `GET /torrents/{id}/extra_files` with full call chain (HTTP → Api → ManagedTorrent → TorrentStorage) |
| **#560** | Torrent creation | `watch::Sender` in `CreateTorrentOptions.on_progress`, drop-based cancellation, `CreateTorrentResult` return type |

> [!IMPORTANT]
> **#560 note**: Our fork uses `AbortHandle::abort()` + `yield_now().await` for cancellation internally. The upstream PR uses the simpler drop-based pattern as ikatson suggested — the abort mechanism is a fork-only enhancement for the queue system.

### ⏳ Pending First Review

| PR | Title | Issues Addressed |
|---|---|---|
| **#563** | Portable mode | #465 |
| **#564** | System tray + autostart | #440 |
| **#565** | FILE_SHARE_DELETE (Windows) | #369, #120, #192 |
| **#567** | Skip redundant set_len | #520 (mtime preservation) |
| **#571** | File integrity check | #510, #509 — simpler than community #510 approach |

### ❌ Closed / ✅ Merged

| PR | Title | Status | Reason |
|---|---|---|---|
| **#555** | 414 URI Too Long | ✅ Merged | — |
| **#566** | ErrorBoundary (WebUI) | ❌ Closed | No concrete crash scenario — closed to focus on real problems |

### 🟢 Possible Future PRs

| Feature | Community Issues | Approach |
|---|---|---|
| `choose_piece_length` | — | Present data on tracker overhead for large content |
| CopyMagnetButton | — | `navigator.clipboard` for WebUI |
| `seek_write` retry (Windows) | — | 3x backoff for transient `ERROR_ACCESS_DENIED` in `opened_file.rs` — defense-in-depth, separate from #556 fix |

### 🔴 Fork-Only (Do Not Submit)

| Feature | Reason |
|---|---|
| `kill_locking_processes` | Uses `taskkill /F` — too aggressive |
| `skip_hash_check` | Risk of data corruption |
| `added_at` / `last_activity` / `total_fetched_bytes` | Fork metadata — upstream may design differently |

---

## BEP52 (BitTorrent v2) Strategy 🎯

| Item | Status | Notes |
|---|---|---|
| [#546](https://github.com/ikatson/rqbit/issues/546) Design | Active | ikatson approved incremental |
| [#568](https://github.com/ikatson/rqbit/pull/568) Merkle tree | ⚠️ Restructure requested | Top-down, no unused code, lazy memory |
| [#554](https://github.com/ikatson/rqbit/pull/554) Phase 2 hybrid | Open | — |
| Our involvement | Commented Feb 11 | Offered production testing |
| Timeline | 3-6 months | — |

### ikatson's BEP52 Vision

> Start with torrent metainfo/info **parsing** v2 fields. Then progressively implement top-down:
> info parsing → optionally fetching piece layers → validating data on disk → downloading missing pieces.
> No standalone modules. No unused code. Lazy memory.

### Why It Matters

**File-level dedup** — game updates change ~10-20% of files. BEP52 merkle roots let us skip unchanged files. Hybrid torrents work with ALL clients in the same swarm. No tracker changes needed.

---

## Community Alignment

| Community Request | Our Action | Status |
|---|---|---|
| #525/#520: FD exhaustion | PR #556 | ⏳ Awaiting re-review (mode tracking added) |
| #520: mtime reset on restart | PR #567 | ⏳ Pending review |
| #311: Peer memory | PR #557 | ⏳ Awaiting re-review |
| #335: Sync extra files | PR #559 | ⏳ Awaiting re-review |
| #333/#524: Torrent creation | PR #560 | ⏳ Awaiting re-review |
| #465: Portable mode | PR #563 | ⏳ Pending review |
| #440: System tray | PR #564 | ⏳ Pending review |
| #369/#120/#192: File locking | PR #565 | ⏳ Pending review |
| #136/#509: Read-only seeding | PR #570 | ⏳ Awaiting re-review (coordination note added) |
| #510: On-disk data verify | PR #571 | ⏳ Pending review |
| #366: 414 URI Too Long | PR #555 | ✅ Merged |
| #546: BEP52 | Tracking | meatsquirk building |

---

## Key Lessons from ikatson Reviews

1. **Actively reviewing** — 6 PRs got feedback on Feb 14 alone.
2. **"No dead code" is a hard rule** — enforced on both us (#559) and meatsquirk (#568).
3. **Unit tests mandatory for AI code** — met with 4 tests on #557.
4. **API design matters** — caller ergonomics (receiver in options, not return).
5. **Top-down preferred** — start from callers, add primitives only as needed.
6. **Close PRs that don't solve real problems** — #566 closed proactively.

## Recent Fork Changes

| Change | Description |
|---|---|
| **LRU cache mode tracking** | Fixed `get_or_open()` to track `(Arc<File>, bool)` — evicts read-only handles when write requested |
| **Handle-eviction retry** | `pwrite_all()`/`pwrite_all_vectored()` retry on Access Denied with cache eviction |
| **`seek_write` retry** | `opened_file.rs` retries 3x with backoff for transient `ERROR_ACCESS_DENIED` on Windows |
| **FATAL error context** | Structured fields (`piece`, `chunk_offset`, `chunk_size`, `peer`) added to FATAL errors |
| **Fork rebased** | Rebased onto upstream [`v9.0.1`](https://github.com/ikatson/rqbit/releases/tag/v9.0.1) (`a499d2f2`) — 11 feature commits + 1 build-fix commit |
| **PR #557 fixed** | Race condition, sort, unit tests — all Feb 14 feedback addressed |
| **PR #559 fixed** | Added `GET /torrents/{id}/extra_files` API endpoint |
| **PR #560 fixed** | `watch::Sender` in options, drop-based cancellation, `CreateTorrentResult` |
| **PR #566 closed** | No concrete crash scenario — closed to focus on real PRs |
| **PR #570 submitted** | Read-only fallback for locked completed files |
| **PR #571 submitted** | File integrity check before trusting fastresume |

---

## Branch model: fork product vs upstream PRs

| Branch | Role |
|---|---|
| `main` | **Fork product** — one squashed commit on top of `upstream/main` (all iPLAYCAFE features + fork CI). Re-squash after each upstream rebase. Do not open this as one giant upstream PR. |
| `feat/*`, `fix/*`, `perf/*` | **Upstream-ready slices** — one feature each, rebased onto `upstream/main`, force-pushed to `origin`. Open PRs to `ikatson/rqbit` from these when ready. |

Do not merge fork-only commits into a PR branch. Keep PR branches ahead of / based on upstream, not on fork `main`.

## Commit hygiene on `main`

Keep **one commit** between `upstream/main` and fork `main` so each upstream release rebase stays fast:

```bash
git fetch upstream --tags
git checkout main
git rebase upstream/main
# resolve conflicts, then verify:
cargo check && cargo clippy --all-targets
# collapse fork delta back to one commit:
git reset --soft upstream/main
git commit -m "feat(fork): iPLAYCAFE enhancements on <upstream-tag>"
# main is PR-protected — push to a sync branch and open PR to main:
git push -u origin HEAD:fork/sync-<upstream-tag>
```

`feat/*` branches stay **one commit each** for upstream PRs; do not squash them into `main`.

## Keeping in sync with upstream

When you choose to sync (no scheduled automation):

```bash
git fetch upstream --tags
git checkout main && git pull origin main
git rebase upstream/main
# resolve conflicts, then verify and re-squash (see "Commit hygiene on main" above)
```

After re-squash, open a PR from `fork/sync-<tag>` → `main` (or temporarily loosen branch protection for one force-push).

Feature branches intended for upstream PRs should be rebased onto `upstream/main` (not fork `main`), one commit each, then force-pushed to `origin`.

## CI on this fork (Ubicloud + Windows hosted)

| Job | `runs-on` | Notes |
|---|---|---|
| Linux check / test / Docker / Linux release | `ubicloud` / `ubicloud-arm` | Default CI path |
| Windows test / Windows release | `windows-latest` | GitHub-hosted — Ubicloud has no Windows |
| macOS release | not in CI | No workflow — build locally on macOS if ever needed |

Docs: [Quickstart](https://www.ubicloud.com/docs/github-actions-integration/quickstart), [Runner types](https://www.ubicloud.com/docs/github-actions-integration/runner-types).

Prerequisite: [Ubicloud Managed Runners](https://console.ubicloud.com) GitHub App connected to `iPLAYCAFE/rqbit` (org already has `ubicloud-managed-runners` installed). First-time forks may need **Actions → "I understand my workflows, go ahead and enable them"** so push triggers run automatically.

## Branch protection on `main`

- Required status checks: all `Run tests` jobs (rust-compat ×2, Linux, Windows, desktop-check)
- Pull request required before merge (0 approvals — solo operator can self-merge after CI)
- `enforce_admins: true` — admins must pass CI too; no force-push

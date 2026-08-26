# Agent guide — iPLAYCAFE/rqbit fork

Read this file first in any new AI session (Cursor, Claude Code, Codex). It is the canonical fork context; `CLAUDE.md` covers upstream architecture and build commands.

## Repository

| Item | Value |
|---|---|
| Fork | [github.com/iPLAYCAFE/rqbit](https://github.com/iPLAYCAFE/rqbit) |
| Upstream | [github.com/ikatson/rqbit](https://github.com/ikatson/rqbit) |
| Fork point | [`v9.0.1`](https://github.com/ikatson/rqbit/releases/tag/v9.0.1) (`a499d2f2`) |
| `main` shape | **One squashed commit** above `upstream/main` (entire fork delta) |

### Remotes

```bash
origin   → https://github.com/iPLAYCAFE/rqbit.git
upstream → https://github.com/ikatson/rqbit.git
```

- **Do not** use or reintroduce Forgejo / `code.iplaycafe.dev` (retired).

## User preferences

- Prefers **Thai** for discussion; ask before irreversible work
- After direction is approved, expects **high operator autonomy** (CLI, MCP, API, browser)
- **Do not** open PRs to upstream `ikatson/rqbit` unless explicitly requested
- **Do not** set up recurring upstream sync automation unless asked
- **Do not** open contribution PRs back to upstream unless requested

## Branch model

| Branch | Purpose |
|---|---|
| `main` | Fork product — single squashed commit with all iPLAYCAFE features + fork CI. **Never** send as one upstream PR. |
| `feat/*`, `fix/*`, `perf/*` | Upstream-ready slices — **one commit each**, based on `upstream/main`, not on fork `main` |

### Upstream PR branches (on `origin`)

`feat/peer-pruning`, `feat/portable-mode`, `feat/sync-extra-files`, `feat/system-tray`, `feat/torrent-creation`, `feat/read-only-fallback`, `feat/file-integrity-monitor`, `feat/file-integrity-check`, `fix/file-share-delete`, `fix/readonly-fallback`, `perf/lazy-file-init`, `perf/lru-file-handle-cache`

Rebase each onto `upstream/main` before opening upstream PRs. See `README.feature_strategy.md` for PR status and ikatson review notes.

## Changing `main` (branch protection)

`main` requires **PR + green CI** (`enforce_admins: true`, no force-push).

1. Branch from `main` → implement → open PR → merge when CI passes.

### After upstream release (rebase + re-squash)

```bash
git fetch upstream --tags
git checkout main && git pull origin main
git rebase upstream/main
# resolve conflicts; then:
cargo check && cargo clippy --all-targets

git reset --soft upstream/main
git commit -m "feat(fork): iPLAYCAFE enhancements on <upstream-tag>"

# Cannot force-push main — push to a sync branch and PR:
git push -u origin HEAD:fork/sync-<tag>
# Open PR fork/sync-<tag> → main, merge when CI green
```

Alternative: temporarily loosen branch protection for one `git push --force-with-lease origin main` (operator only).

## CI (fork)

Workflow: `.github/workflows/test.yml`

| Job | Runner |
|---|---|
| `check-rust-compat` (1.94, 1.97) | `ubicloud` |
| `test (ubicloud, linux)` | `ubicloud` |
| `test (windows-latest, windows)` | `windows-latest` (excludes `rqbit-desktop` crate tests) |
| `test (macos-latest, macos)` | `macos-latest` (GitHub-hosted; excludes `rqbit-desktop`) |
| `desktop-check` | `windows-latest` (npm build + `cargo check -p rqbit-desktop`) |

- Linux releases / Docker: `release-linux.yml` on Ubicloud
- Windows release: `release-windows.yml` on `windows-latest`
- macOS release: not in CI yet (tests only on `macos-latest`)
- Ubicloud app `ubicloud-managed-runners` installed on org `iPLAYCAFE`

## Fork features (not in upstream)

LRU file-handle cache, lazy file init, peer pruning, sync extra files, Windows Restart Manager / `FILE_SHARE_DELETE`, torrent creation queue, portable mode, system tray, file integrity monitor (FIM), read-only fallback, webui create-torrent UX, fork metadata fields (`added_at`, etc.), and related API/desktop changes.

**Fork-only (do not upstream):** `kill_locking_processes`, `skip_hash_check`, aggressive fork metadata.

## Key documentation

| File | Contents |
|---|---|
| `README.md` | Full fork delta vs upstream (modules, API, desktop, webui) |
| `README.feature_strategy.md` | Upstream PR strategy, community alignment, CI, branch protection |
| `README.api.md` / `README.cli.md` | API and CLI reference (fork-extended) |
| `CLAUDE.md` | Build commands, architecture, Rust/webui rules |
| `crates/librqbit/webui/CLAUDE.md` | WebUI patterns |

## Build & verify (quick)

```bash
cargo check && cargo clippy --all-targets
cargo test                                    # Linux default members
cargo test --workspace --exclude rqbit-desktop  # Windows-style workspace tests

cd desktop && npm install && npm run build    # frontend
cargo check -p rqbit-desktop                  # desktop Rust

npm run format   # after TS/TSX edits (repo root)
```

Desktop full build: `cd desktop && npm run tauri build` (expensive; local/release only).

## Agent rules

- Run `cargo check` / `cargo clippy` after Rust changes; run tests before claiming done
- Run `npm run format` after webui/desktop TypeScript changes
- Use `rg` not `grep`; never read multi-GB log files without `rg | head`
- Minimize diff scope; match existing code style
- BEP 52 / torrent code: use v2 structures per spec
- Only create git commits when the user asks (except this docs PR flow)

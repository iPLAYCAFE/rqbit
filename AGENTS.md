## Learned User Preferences

- Prefers Thai for discussion; welcomes clarifying questions and better alternatives before irreversible work
- Once a direction is approved, expects high operator autonomy on this fork (CLI/MCP/API/browser as appropriate)
- Do not open contribution PRs back to upstream `ikatson/rqbit` unless explicitly requested
- Do not set up a recurring upstream sync cadence unless asked
- Fork CI should use Ubicloud runners (`runs-on: ubicloud`); if Windows jobs are required, GitHub-hosted runners are acceptable
- Keep fork `main` as one squashed commit above `upstream/main`; use `feat/*` branches for upstream PR slices

## Learned Workspace Facts

- This workspace is the GitHub fork `iPLAYCAFE/rqbit` of upstream `ikatson/rqbit`
- Forgejo / `code.iplaycafe.dev` is retired; do not use or reintroduce that remote
- Fork carries custom features beyond upstream (FIM, portable/tray, torrent creation, peer pruning, LRU cache, Windows Restart Manager / share-delete, and related UX)
- `main` was rebased onto upstream `v9.0.1` with the fork-only commits preserved as a linear history

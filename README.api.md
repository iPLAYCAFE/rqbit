# rqbit HTTP API Reference (iPLAYCAFE Fork)

> **Complete reference for all available HTTP API endpoints.**
> Based on source: `crates/librqbit/src/http_api/` — rqbit v9.0.0-beta.2
>
> Base URL: `http://<listen_addr>/` (default `http://127.0.0.1:3030/` for server mode)

---

## Table of Contents

- [Authentication](#authentication)
- [CORS Configuration](#cors-configuration)
- [Path Parameters](#path-parameters)
- [Quick Reference](#quick-reference)
- [Endpoints — Read-Only](#endpoints--read-only)
  - [Root & Discovery](#root--discovery)
  - [Torrent Management](#torrent-management)
  - [Torrent Stats](#torrent-stats)
  - [Streaming & Playlists](#streaming--playlists)
  - [DHT](#dht)
  - [Session & Configuration](#session--configuration)
  - [Logging](#logging)
  - [Prometheus Metrics](#prometheus-metrics)
- [Endpoints — Read-Write](#endpoints--read-write)
  - [Torrent Actions](#torrent-actions)
  - [Create Torrent](#create-torrent)
  - [Create Torrent Task Queue (Fork)](#create-torrent-task-queue-fork)
  - [Extra Files Management (Fork)](#extra-files-management-fork)
  - [Configuration](#configuration)
- [Response Types](#response-types)
- [Error Handling](#error-handling)

---

## Authentication

HTTP Basic Authentication is supported. Set the environment variable:

```
RQBIT_HTTP_BASIC_AUTH_USERPASS=username:password
```

When enabled, all API requests must include an `Authorization` header:

```
Authorization: Basic <base64(username:password)>
```

Unauthenticated requests receive a `401 Unauthorized` with `WWW-Authenticate: Basic realm="API"`.

---

## CORS Configuration

The API allows CORS from the following origins by default:

| Origin | Purpose |
|---|---|
| `http://localhost:3031` | Web UI development |
| `http://127.0.0.1:3031` | Web UI development |
| `http://localhost:1420` | Tauri desktop app (dev) |
| `tauri://localhost` | Tauri desktop app (prod) |

Additional origins can be allowed via the `CORS_ALLOW_REGEXP` environment variable (regex pattern).

---

## Path Parameters

Throughout this document, `{id}` refers to a **torrent identifier** which can be:

| Format | Example | Description |
|---|---|---|
| Integer ID | `0`, `1`, `42` | Numeric torrent ID assigned by the session |
| Info Hash | `a1b2c3d4e5f6...` (40 hex chars) | The 20-byte SHA-1 info hash of the torrent |

---

## Quick Reference

### Read-Only Endpoints (always available)

| Method | Path | Description |
|---|---|---|
| `GET` | `/` | API root — list all available endpoints |
| `GET` | `/torrents` | List all torrents |
| `GET` | `/torrents/{id}` | Torrent details (files, metadata) |
| `GET` | `/torrents/{id}/stats` | Torrent live stats (v0, legacy) |
| `GET` | `/torrents/{id}/stats/v1` | Torrent stats (v1, full) |
| `GET` | `/torrents/{id}/haves` | Piece bitfield (SVG or binary) |
| `GET` | `/torrents/{id}/metadata` | Download `.torrent` file |
| `GET` | `/torrents/{id}/peer_stats` | Per-peer statistics |
| `GET` | `/torrents/{id}/peer_stats/prometheus` | Per-peer stats (Prometheus) |
| `GET` | `/torrents/{id}/stream/{file_id}` | Stream a file |
| `GET` | `/torrents/{id}/stream/{file_id}/{filename}` | Stream a file (with filename) |
| `GET` | `/torrents/{id}/playlist` | M3U8 playlist for a torrent |
| `GET` | `/torrents/playlist` | M3U8 playlist for all torrents |
| `GET` | `/torrents/limits` | Get current rate limits |
| `GET` | `/dht/stats` | DHT statistics |
| `GET` | `/dht/table` | DHT routing table |
| `GET` | `/stats` | Global session statistics |
| `GET` | `/stream_logs` | Stream logs (SSE) |
| `GET` | `/metrics` | Prometheus metrics (if enabled) |
| `GET` | `/web/` | Web UI (if enabled) |

### Read-Write Endpoints (server mode only)

| Method | Path | Description |
|---|---|---|
| `POST` | `/torrents` | Add a torrent |
| `POST` | `/torrents/{id}/pause` | Pause a torrent |
| `POST` | `/torrents/{id}/start` | Resume a torrent |
| `POST` | `/torrents/{id}/forget` | Remove torrent, keep files |
| `POST` | `/torrents/{id}/delete` | Remove torrent and files |
| `POST` | `/torrents/{id}/update_only_files` | Change file selection |
| `POST` | `/torrents/{id}/add_peers` | Add peers manually |
| `POST` | `/torrents/create` | Create and seed a torrent |
| `POST` | `/torrents/create_task` | Enqueue create torrent task |
| `GET` | `/torrents/create_tasks` | List create torrent tasks |
| `DELETE` | `/torrents/create_tasks/{id}` | Cancel create torrent task |
| `GET` | `/torrents/{id}/extra_files` | List extra files in torrent dir |
| `POST` | `/torrents/{id}/delete_extra_files` | Delete extra files |
| `POST` | `/torrents/limits` | Update rate limits |
| `POST` | `/torrents/resolve_magnet` | Resolve magnet to torrent |
| `POST` | `/rust_log` | Change log filter at runtime |

---

## Endpoints — Read-Only

### Root & Discovery

#### `GET /`

List all available API endpoints. If the request `Accept` header contains `text/html` and the web UI is enabled, redirects to `/web/`.

**Response**: `200 OK` — `application/json`

```json
{
  "apis": {
    "GET /": "list all available APIs",
    "GET /dht/stats": "DHT stats",
    "...": "..."
  },
  "server": "rqbit",
  "version": "9.0.0-beta.2"
}
```

---

### Torrent Management

#### `GET /torrents`

List all managed torrents with basic metadata.

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `with_stats` | `bool` | `false` | Include stats for each torrent in the response. File progress is omitted in list view for performance. |

**Response**: `200 OK`

```json
{
  "torrents": [
    {
      "id": 0,
      "info_hash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
      "name": "My Torrent",
      "output_folder": "/downloads",
      "total_pieces": 1234,
      "stats": null
    }
  ]
}
```

> **Note**: `files` and `trackers` fields are always `null` in list view for performance optimization. Use `GET /torrents/{id}` for full details.

---

#### `GET /torrents/{id}`

Get detailed information about a specific torrent, including file list and tracker URLs.

**Response**: `200 OK`

```json
{
  "id": 0,
  "info_hash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
  "name": "My Torrent",
  "output_folder": "/downloads",
  "total_pieces": 1234,
  "files": [
    {
      "name": "movie.mkv",
      "components": ["movie.mkv"],
      "length": 1073741824,
      "included": true,
      "attributes": {}
    }
  ],
  "trackers": [
    "udp://tracker.example.com:1337/announce"
  ]
}
```

---

#### `GET /torrents/{id}/haves`

Get the piece bitfield showing download progress.

**Request Headers**:

| Header | Value | Description |
|---|---|---|
| `Accept` | `application/octet-stream` | Return raw binary bitfield |
| `Accept` | *(other)* | Return SVG visualization (default) |

**Response (SVG — default)**: `200 OK` — `image/svg+xml`

Returns an SVG image where green (`#22c55e`) = have, gray (`#374151`) = missing.

**Response (binary)**: `200 OK` — `application/octet-stream`

Returns raw bitfield bytes with a custom header:

| Header | Description |
|---|---|
| `X-Bitfield-Len` | Total number of pieces |

---

#### `GET /torrents/{id}/metadata`

Download the `.torrent` file for a managed torrent.

**Response**: `200 OK`

| Header | Value |
|---|---|
| `Content-Disposition` | `attachment; filename="<name>.torrent"` |

Body contains the raw torrent file bytes.

---

### Torrent Stats

#### `GET /torrents/{id}/stats` (v0 — Legacy)

Get live statistics for a torrent. Only works when the torrent is in a "live" state (downloading/seeding).

**Response**: `200 OK` — Live stats JSON (legacy format).

Returns `412 Precondition Failed` if the torrent is not live.

---

#### `GET /torrents/{id}/stats/v1`

Get comprehensive statistics for a torrent (works in all states).

**Response**: `200 OK`

```json
{
  "state": "live",
  "finished": false,
  "total_bytes": 1073741824,
  "progress_bytes": 536870912,
  "file_progress": [...],
  "live": {
    "snapshot": {
      "uploaded_bytes": 1048576,
      "peer_stats": {
        "live": 5,
        "connecting": 2,
        "queued": 10,
        "dead": 3,
        "seen": 50
      }
    }
  }
}
```

---

#### `GET /torrents/{id}/peer_stats`

Get per-peer statistics.

**Query Parameters**:

| Parameter | Type | Default | Values | Description |
|---|---|---|---|---|
| `state` | `string` | `live` | `live`, `all` | Filter peers by state |

**Response**: `200 OK`

```json
{
  "peers": {
    "192.168.1.10:51413": {
      "counters": {
        "incoming_connections": 0,
        "fetched_bytes": 10485760,
        "uploaded_bytes": 524288,
        "total_time_connecting_ms": 150,
        "connection_attempts": 1,
        "connections": 1,
        "errors": 0,
        "fetched_chunks": 640,
        "downloaded_and_checked_pieces": 40,
        "total_piece_download_ms": 5000,
        "times_stolen_from_me": 0,
        "times_i_stole": 2
      },
      "state": "live",
      "conn_kind": "tcp"
    }
  }
}
```

---

#### `GET /torrents/{id}/peer_stats/prometheus`

Get per-peer download statistics in Prometheus exposition format. Only includes peers that have fetched ≥ 1 MiB.

**Response**: `200 OK` — `text/plain`

```
# TYPE rqbit_peer_fetched_bytes counter
rqbit_peer_fetched_bytes{addr="192.168.1.10:51413"} 9437184
```

---

### Streaming & Playlists

#### `GET /torrents/{id}/stream/{file_id}`
#### `GET /torrents/{id}/stream/{file_id}/{filename}`

Stream a file from a torrent. Supports HTTP Range requests for seeking. The `{filename}` variant is useful for media players that infer file type from the URL.

**Request Headers**:

| Header | Example | Description |
|---|---|---|
| `Range` | `bytes=0-1023` | Request a specific byte range |
| `transferMode.dlna.org` | `Streaming` | DLNA streaming mode |
| `getcontentFeatures.dlna.org` | `1` | Request DLNA content features |

**Response**:

| Status | Condition |
|---|---|
| `200 OK` | Full file response |
| `206 Partial Content` | Range request |
| `416 Range Not Satisfiable` | Invalid range |

**Response Headers**:

| Header | Description |
|---|---|
| `Accept-Ranges` | Always `bytes` |
| `Content-Length` | Size of the response body |
| `Content-Range` | `bytes start-end/total` (for 206) |
| `Content-Type` | MIME type (auto-detected) |
| `contentFeatures.dlna.org` | `DLNA.ORG_OP=01` (if requested) |

---

#### `GET /torrents/{id}/playlist`

Generate an M3U8 playlist for all playable media files (audio/video) in a specific torrent.

**Response**: `200 OK`

| Header | Value |
|---|---|
| `Content-Type` | `application/mpegurl; charset=utf-8` |
| `Content-Disposition` | `attachment; filename="rqbit-playlist.m3u8"` |

```
#EXTM3U
http://host:port/torrents/0/stream/0/movie.mkv
http://host:port/torrents/0/stream/2/audio.mp3
```

> **Note**: Requires the `Host` header. Only includes files with recognized audio/video MIME types, sorted alphabetically.

---

#### `GET /torrents/playlist`

Generate an M3U8 playlist for all playable media files across all managed torrents.

Same format and headers as the per-torrent playlist.

---

### DHT

#### `GET /dht/stats`

Get DHT (Distributed Hash Table) statistics.

**Response**: `200 OK` — DHT stats JSON

Returns `501 Not Implemented` if DHT is disabled.

---

#### `GET /dht/table`

Get the full DHT routing table.

**Response**: `200 OK`

```json
{
  "v4": { ... },
  "v6": { ... }
}
```

Returns `501 Not Implemented` if DHT is disabled.

---

### Session & Configuration

#### `GET /stats`

Get global session statistics including aggregate download/upload speeds, total bytes, and peer counts.

**Response**: `200 OK` — Session stats snapshot JSON

---

#### `GET /torrents/limits`

Get the current session rate limits.

**Response**: `200 OK`

```json
{
  "upload_bps": 1048576,
  "download_bps": null
}
```

`null` means unlimited.

---

### Logging

#### `GET /stream_logs`

Stream server log lines in real-time as a Server-Sent Events (SSE) stream.

**Response**: `200 OK` — Streaming body of log line bytes

Connection stays open and continuously delivers new log lines.

---

### Prometheus Metrics

#### `GET /metrics`

> Requires `prometheus` compile-time feature (enabled by default)

Render Prometheus metrics including session-level statistics.

**Response**: `200 OK` — `text/plain` (Prometheus exposition format)

---

## Endpoints — Read-Write

These endpoints are only available when the API is in **read-write mode** (server mode). They return `405 Method Not Allowed` in read-only mode.

### Torrent Actions

#### `POST /torrents`

Add a new torrent to the session.

**Request Headers**:

| Header | Value | Description |
|---|---|---|
| `X-Timeout` | milliseconds | Custom timeout (default: 600000ms, max: 3600000ms) |

**Request Body**: Raw bytes — one of:
- A magnet link (UTF-8 string starting with `magnet:`)
- An HTTP/HTTPS URL to a `.torrent` file (UTF-8 string)
- Raw `.torrent` file bytes

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `overwrite` | `bool` | `false` | Allow writing on top of existing files |
| `output_folder` | `string` | Session default | Override the output folder for this torrent |
| `sub_folder` | `string` | — | Sub-folder within the output folder |
| `only_files_regex` | `string` | — | Regex to filter which files to download |
| `only_files` | `string` | — | Comma-separated list of file indices to download (e.g., `0,1,3`) |
| `peer_connect_timeout` | `u64` | — | Peer connect timeout in seconds |
| `peer_read_write_timeout` | `u64` | — | Peer read/write timeout in seconds |
| `initial_peers` | `string` | — | Comma-separated initial peers (`host:port`) |
| `is_url` | `bool` | Auto-detect | Force interpreting the body as a URL (`true`) or torrent bytes (`false`) |
| `list_only` | `bool` | `false` | Only resolve metadata without adding the torrent |
| `skip_initial_check` | `bool` | `false` | Skip initial hash check on startup |
| `sync_extra_files` | `bool` | — | Enable extra file sync feature for this torrent |

**Response**: `200 OK`

```json
{
  "id": 0,
  "details": {
    "id": 0,
    "info_hash": "...",
    "name": "My Torrent",
    "output_folder": "/downloads",
    "total_pieces": 1234,
    "files": [...],
    "trackers": [...]
  },
  "output_folder": "/downloads",
  "seen_peers": null,
  "already_managed": false
}
```

If the torrent is already managed, `already_managed` will be `true`.

---

#### `POST /torrents/resolve_magnet`

Resolve a magnet link to torrent metadata without adding it to the session.

**Request Headers**:

| Header | Value | Description |
|---|---|---|
| `Accept` | `application/json` | Return decoded torrent metadata as JSON |
| `Accept` | *(other)* | Return raw `.torrent` file bytes |
| `X-Timeout` | milliseconds | Custom timeout (default: 600000ms, max: 3600000ms) |

**Request Body**: UTF-8 string — the magnet link or URL

**Response (torrent bytes — default)**: `200 OK`

| Header | Value |
|---|---|
| `Content-Type` | `application/x-bittorrent` |
| `Content-Disposition` | `attachment; filename="<name>.torrent"` |

**Response (JSON)**: `200 OK` — `application/json`

Returns the decoded bencode data of the torrent file as JSON.

---

#### `POST /torrents/{id}/pause`

Pause a torrent. Stops all peer connections.

**Response**: `200 OK` — `{}`

---

#### `POST /torrents/{id}/start`

Resume a paused torrent.

**Response**: `200 OK` — `{}`

---

#### `POST /torrents/{id}/forget`

Remove a torrent from the session. **Downloaded files are kept** on disk.

**Response**: `200 OK` — `{}`

---

#### `POST /torrents/{id}/delete`

Remove a torrent from the session **and delete all downloaded files** from disk.

**Response**: `200 OK` — `{}`

---

#### `POST /torrents/{id}/update_only_files`

Change the selection of files to download within a torrent.

**Request Body**: `application/json`

```json
{
  "only_files": [0, 1, 3]
}
```

`only_files` is an array of zero-based file indices to download.

**Response**: `200 OK` — `{}`

---

#### `POST /torrents/{id}/add_peers`

Manually add peers to a torrent.

**Request Body**: `text/plain` — Newline-delimited list of peer addresses

```
192.168.1.10:51413
10.0.0.5:6881
```

**Response**: `200 OK`

```json
{
  "added": 2
}
```

Only peers not already seen are counted as `added`.

---

### Create Torrent

#### `POST /torrents/create`

Create a `.torrent` file from a local path and start seeding it.

> **Requires** `--http-api-allow-create` CLI flag. Returns `403 Forbidden` otherwise.

**Request Body**: `text/plain` — UTF-8 path to the local file or directory

**Query Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `output` | `string` | `magnet` | Output format: `magnet` or `torrent` |
| `name` | `string` | — | Custom torrent name |
| `trackers` | `string[]` | — | Tracker URLs to announce to |
| `stream` | `bool` | `false` | Return progress as a streaming response |

**Response (output=magnet)**: `200 OK` — `text/plain`

Returns the magnet link as plain text.

**Response Headers** (for both output modes):

| Header | Description |
|---|---|
| `torrent-id` | The assigned torrent ID |
| `torrent-info-hash` | The info hash |

**Response (output=torrent)**: `200 OK`

| Header | Value |
|---|---|
| `Content-Type` | `application/x-bittorrent` |
| `Content-Disposition` | `attachment; filename="<name>.torrent"` |

**Response (stream=true)**: `200 OK` — Streaming body

Newline-delimited JSON events:

```json
{"type": "success", "id": 0}
```
or on error:
```json
{"type": "error", "error": "error message"}
```

---

### Create Torrent Task Queue (Fork)

These endpoints manage background torrent creation tasks with cancellation support.

#### `POST /torrents/create_task`

Enqueue a torrent creation task to run in the background.

> **Requires** `--http-api-allow-create` CLI flag.

**Request Body**: `text/plain` — UTF-8 path to the local file or directory

**Query Parameters**: Same as `POST /torrents/create` (except `stream`).

**Response**: `200 OK`

```json
{
  "id": 0
}
```

---

#### `GET /torrents/create_tasks`

List all pending/running/completed create torrent tasks.

**Response**: `200 OK` — JSON array of task objects

```json
[
  {
    "id": 0,
    "state": "running",
    "path": "/path/to/files",
    "...": "..."
  }
]
```

---

#### `DELETE /torrents/create_tasks/{id}`

Cancel a running create torrent task.

**Response**: `200 OK`

```json
{
  "ok": true
}
```

---

### Extra Files Management (Fork)

These endpoints manage "extra" files in a torrent's output directory — files that exist on disk but are not part of the torrent manifest.

#### `GET /torrents/{id}/extra_files`

List files in the torrent's output directory that are not part of the torrent.

**Response**: `200 OK`

```json
{
  "extra_files": [
    "Thumbs.db",
    "desktop.ini",
    ".DS_Store"
  ]
}
```

---

#### `POST /torrents/{id}/delete_extra_files`

Delete specific extra files from the torrent's output directory.

**Request Body**: `application/json`

```json
{
  "files": ["Thumbs.db", "desktop.ini"]
}
```

**Response**: `200 OK`

```json
{
  "removed": 2,
  "failed": 0
}
```

---

### Configuration

#### `POST /torrents/limits`

Update session rate limits at runtime.

**Request Body**: `application/json`

```json
{
  "upload_bps": 1048576,
  "download_bps": 5242880
}
```

Both fields are optional. Use `null` to remove a limit (unlimited).

| Field | Type | Description |
|---|---|---|
| `upload_bps` | `u32 \| null` | Upload rate limit in bytes per second |
| `download_bps` | `u32 \| null` | Download rate limit in bytes per second |

**Response**: `200 OK` — `{}`

---

#### `POST /rust_log`

Change the `RUST_LOG` filter directive at runtime for debugging.

**Request Body**: `text/plain` — The new `RUST_LOG` value

```
librqbit=debug,info
```

**Response**: `200 OK` — `{}`

---

## Response Types

### TorrentDetailsResponse

```json
{
  "id": 0,
  "info_hash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
  "name": "My Torrent",
  "output_folder": "/downloads/My Torrent",
  "total_pieces": 1234,
  "files": [
    {
      "name": "path/to/file.mkv",
      "components": ["path", "to", "file.mkv"],
      "length": 1073741824,
      "included": true,
      "attributes": {}
    }
  ],
  "stats": null,
  "trackers": ["udp://tracker.example.com:1337/announce"]
}
```

### PeerCounters

| Field | Type | Description |
|---|---|---|
| `incoming_connections` | `u32` | Number of incoming connections from this peer |
| `fetched_bytes` | `u64` | Total bytes downloaded from this peer |
| `uploaded_bytes` | `u64` | Total bytes uploaded to this peer |
| `total_time_connecting_ms` | `u64` | Total time spent connecting (ms) |
| `connection_attempts` | `u32` | Number of outgoing connection attempts |
| `connections` | `u32` | Number of successful outgoing connections |
| `errors` | `u32` | Number of errors with this peer |
| `fetched_chunks` | `u32` | Number of chunks fetched |
| `downloaded_and_checked_pieces` | `u32` | Pieces downloaded and hash-verified |
| `total_piece_download_ms` | `u64` | Total piece download time (ms) |
| `times_stolen_from_me` | `u32` | Times another peer stole a piece assignment |
| `times_i_stole` | `u32` | Times we stole a piece assignment from another peer |

### LimitsConfig

```json
{
  "upload_bps": 1048576,
  "download_bps": null
}
```

Values are in bytes per second. `null` means unlimited.

---

## Error Handling

The API returns standard HTTP status codes:

| Status | Description |
|---|---|
| `200 OK` | Success |
| `206 Partial Content` | Successful range request (streaming) |
| `400 Bad Request` | Invalid request parameters |
| `401 Unauthorized` | Authentication required |
| `403 Forbidden` | Operation not permitted (e.g., create without `--http-api-allow-create`) |
| `404 Not Found` | Torrent not found |
| `405 Method Not Allowed` | Write operation on read-only API |
| `408 Request Timeout` | Operation timed out |
| `412 Precondition Failed` | Torrent is not in the required state (e.g., not live) |
| `416 Range Not Satisfiable` | Invalid byte range in stream request |
| `500 Internal Server Error` | Unexpected server error |
| `501 Not Implemented` | Feature not available (e.g., DHT disabled) |

Error responses include a descriptive plain text or JSON body when possible.

### Timeout Header

Some POST endpoints support the `X-Timeout` request header to customize the operation timeout:

| Header | Type | Default | Max | Description |
|---|---|---|---|---|
| `X-Timeout` | `u64` | `600000` | `3600000` | Timeout in milliseconds |

Applies to: `POST /torrents`, `POST /torrents/resolve_magnet`

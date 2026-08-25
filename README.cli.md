# rqbit CLI Reference (iPLAYCAFE Fork)

> **Complete reference for all available command-line options.**
> Based on source: `crates/rqbit/src/main.rs` — rqbit v9.0.0-beta.2

---

## Table of Contents

- [Synopsis](#synopsis)
- [Subcommands](#subcommands)
  - [`server start`](#server-start)
  - [`download`](#download)
  - [`share`](#share)
  - [`completions`](#completions)
- [Global Options](#global-options)
  - [Logging](#logging)
  - [HTTP API](#http-api)
  - [Runtime](#runtime)
  - [DHT (Distributed Hash Table)](#dht-distributed-hash-table)
  - [Peer Connections](#peer-connections)
  - [Networking & Listening](#networking--listening)
  - [UPnP](#upnp)
  - [Rate Limiting](#rate-limiting)
  - [Blocklist / Allowlist](#blocklist--allowlist)
  - [Trackers](#trackers)
  - [Storage](#storage)
  - [Proxy](#proxy)
  - [Miscellaneous](#miscellaneous)
- [Environment-Only Variables](#environment-only-variables)
- [Compile-Time Features](#compile-time-features)
- [Usage Examples](#usage-examples)
- [Default Behavior Notes](#default-behavior-notes)

---

## Synopsis

```
rqbit [GLOBAL OPTIONS] <SUBCOMMAND> [SUBCOMMAND OPTIONS]
```

rqbit is a BitTorrent client that can run as a long-lived **server** with an HTTP API, perform a one-shot **download**, **share** files via torrent, or generate **shell completions**.

---

## Subcommands

### `server start`

Start rqbit as a long-lived server with an HTTP API, session persistence, and automatic torrent management.

```
rqbit [GLOBAL OPTIONS] server start [OPTIONS] <OUTPUT_FOLDER>
```

#### Arguments

| Argument | Required | Description |
|---|---|---|
| `<OUTPUT_FOLDER>` | **Yes** | The output folder to write downloaded files to. Will be created if it does not exist. |

#### Options

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--disable-persistence` | — | `RQBIT_SESSION_PERSISTENCE_DISABLE` | `false` | Disable server persistence. The server will not read or write its state to disk. All torrents will be lost on restart. |
| `--persistence-location <PATH>` | — | `RQBIT_SESSION_PERSISTENCE_LOCATION` | OS-specific folder | The folder to store session data in. If the value starts with `postgres://`, PostgreSQL will be used as the backend instead of a JSON file (requires `postgres` feature). |
| `--fastresume` | — | `RQBIT_FASTRESUME` | `false` | **[Experimental]** If set, will try to resume quickly after restart and skip checksumming. |
| `--watch-folder <PATH>` | — | `RQBIT_WATCH_FOLDER` | — | A folder to watch for added `.torrent` files. All `.torrent` files placed in this folder will be automatically added to the session. |

#### Behavior Notes

- **Listen port**: Defaults to `4240` for the server (if `--listen-port` is not set).
- **HTTP API listen address**: Defaults to `127.0.0.1:3030` for the server (if `--http-api-listen-addr` is not set).
- **HTTP API mode**: The server's HTTP API is **read-write** (not read-only).
- **Persistence**: Enabled by default; uses a JSON file in an OS-specific directory unless overridden by `--persistence-location`.

---

### `download`

Download one or more torrents in a stateless, ephemeral mode. No session persistence is used.

```
rqbit [GLOBAL OPTIONS] download [OPTIONS] <TORRENT_PATH>...
```

#### Arguments

| Argument | Required | Description |
|---|---|---|
| `<TORRENT_PATH>...` | **Yes** | One or more torrent sources. Supported formats: local `.torrent` file path, HTTP/HTTPS URL to a `.torrent` file, or a `magnet:` link. |

#### Options

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--output-folder <PATH>` | `-o` | — | Current directory | The output folder to write downloaded files to. |
| `--sub-folder <PATH>` | `-s` | — | — | A sub-folder within the output folder to write to. Useful when a server is running with `output_folder` configured and you don't want to specify the full path. |
| `--filename-re <REGEX>` | `-r` | — | — | If set, only files whose filename matches this regex will be downloaded. Other files in the torrent will be skipped. |
| `--list` | `-l` | — | `false` | Only list the torrent metadata contents (file names, sizes). Don't download anything. When used, listeners and HTTP API are disabled. |
| `--overwrite` | — | — | `false` | Allow writing on top of existing files. Without this flag, existing files will cause an error. |
| `--exit-on-finish` | `-e` | — | `false` | Exit the program once all torrents complete download. Without this flag, rqbit will continue seeding indefinitely. |
| `--initial-peers <PEERS>` | — | — | — | A comma-separated list of initial peers in `host:port` format (e.g., `192.168.1.10:6881,10.0.0.5:6881`). These peers are contacted immediately without waiting for tracker/DHT responses. |
| `--disable-http-api` | — | — | `false` | Disable the HTTP API entirely for this download session. |

#### Behavior Notes

- **DHT persistence**: Always disabled for `download` (ephemeral mode).
- **Session persistence**: Always disabled for `download`.
- **UPnP port forwarding**: Always disabled for `download` (ephemeral session).
- **HTTP API listen address**: Defaults to `127.0.0.1:<ephemeral port>` (if `--http-api-listen-addr` is not set).
- **HTTP API mode**: Read-only by default.
- When `--list` is used, all networking (listeners, HTTP API) is automatically disabled.

---

### `share`

Create a torrent from a local file or directory and make it available for download by peers. This is a stateless, ephemeral operation.

```
rqbit [GLOBAL OPTIONS] share [OPTIONS] <PATH> [TRACKERS...]
```

#### Arguments

| Argument | Required | Description |
|---|---|---|
| `<PATH>` | **Yes** | The local file or directory path to create a torrent from and share. |
| `[TRACKERS...]` | No | Tracker URLs to announce to (comma-separated). These are appended to any trackers loaded from `--trackers-filename` / `RQBIT_TRACKERS_FILENAME`. Up to 32 tracker URLs. |

#### Options

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--name <NAME>` | `-n` | — | — | Optional torrent name to use in the torrent file and magnet link. If not set, the file/directory name is used. |

#### Behavior Notes

- **Requires a listener**: At least one listener (TCP or uTP) must be enabled. If all listeners are disabled, `share` will error.
- **DHT persistence**: Always disabled (ephemeral mode).
- **Session persistence**: Always disabled.
- **HTTP API listen address**: Defaults to `[::]:0` (all interfaces, ephemeral port) if `--http-api-listen-addr` is not set.
- After creating the torrent, a **magnet link** is printed to stdout.
- A warning is displayed that torrents are **public** — anyone on the network can discover and download the shared content.

---

### `completions`

Generate shell completion scripts for rqbit.

```
rqbit completions <SHELL>
```

#### Arguments

| Argument | Required | Description |
|---|---|---|
| `<SHELL>` | **Yes** | The shell to generate completions for. Supported values: `bash`, `elvish`, `fish`, `powershell`, `zsh`. |

#### Usage

```bash
# Bash
eval "$(rqbit completions bash)"

# Zsh  (add to ~/.zshrc)
eval "$(rqbit completions zsh)"

# Fish
rqbit completions fish | source

# PowerShell
rqbit completions powershell | Out-String | Invoke-Expression
```

---

## Global Options

These options apply to **all subcommands** and must be placed **before** the subcommand.

### Logging

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `-v <LEVEL>` | `-v` | `RQBIT_LOG_LEVEL_CONSOLE` | `info` | The console log level. Possible values: `trace`, `debug`, `info`, `warn`, `error`. |
| `--log-file <PATH>` | — | `RQBIT_LOG_FILE` | — | A log filename to also write to in addition to the console output. |
| `--log-file-rust-log <FILTER>` | — | `RQBIT_LOG_FILE_RUST_LOG` | `librqbit=debug,info` | The `RUST_LOG`-style filter string to use for the log file. Only applies when `--log-file` is set. |

### HTTP API

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--http-api-listen-addr <ADDR>` | — | `RQBIT_HTTP_API_LISTEN_ADDR` | See notes | The socket address (`ip:port`) for the HTTP API to listen on. Ignored if rqbit is passed a socket by systemd (Linux socket activation). See [Default Behavior Notes](#default-behavior-notes) for per-subcommand defaults. |
| `--http-api-allow-create` | — | `RQBIT_HTTP_API_ALLOW_CREATE` | `false` | Allow creating torrents via the HTTP API. |

### Runtime

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--single-thread-runtime` | `-s` | `RQBIT_SINGLE_THREAD_RUNTIME` | `false` | Use tokio's single-threaded runtime. May perform better in some cases. Main purpose is easier debugging with time profilers. |
| `--worker-threads <N>` | `-t` | `RQBIT_RUNTIME_WORKER_THREADS` | Tokio default | How many threads to spawn for the tokio executor. Only applies to multi-threaded runtime. |
| `--max-blocking-threads <N>` | — | `RQBIT_RUNTIME_MAX_BLOCKING_THREADS` | `8` | Maximum number of blocking tokio threads to spawn for disk reads/writes. Higher number = more parallel I/O = more memory usage. The tokio default (512) is too high for this CPU-bound workload. |

### DHT (Distributed Hash Table)

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--disable-dht` | — | `RQBIT_DHT_DISABLE` | `false` | Disable DHT entirely. Peer discovery will rely solely on trackers and other mechanisms. |
| `--disable-dht-persistence` | — | `RQBIT_DHT_PERSISTENCE_DISABLE` | `false` | Disable DHT state reading and storing. Useful as a workaround when launching multiple rqbit instances to avoid DHT port conflicts. Automatically enabled for `download` and `share` subcommands. |
| `--dht-bootstrap-addrs <ADDRS>` | — | `RQBIT_DHT_BOOTSTRAP` | Built-in defaults | A comma-separated list of `host:port` or `ip:port` addresses for DHT bootstrap nodes. |

### Peer Connections

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--peer-connect-timeout <DURATION>` | — | `RQBIT_PEER_CONNECT_TIMEOUT` | `10s` | The timeout for connecting to peers. Accepts duration strings like `1s`, `1.5s`, `100ms`, `10s`. |
| `--peer-read-write-timeout <DURATION>` | — | `RQBIT_PEER_READ_WRITE_TIMEOUT` | `150s` | The timeout for `read()` and `write()` operations on peer connections. Accepts duration strings. |
| `--peer-limit <N>` | — | `RQBIT_PEER_LIMIT` | — | The maximum number of connected peers per torrent. If not set, uses the library default. |

### Networking & Listening

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--listen-port <PORT>` | — | `RQBIT_LISTEN_PORT` | See notes | The port to listen for incoming peer connections (applies to both TCP and uTP). Defaults to `4240` for `server start`, and an ephemeral port for `download` / `share`. |
| `--announce-port <PORT>` | — | `RQBIT_ANNOUNCE_PORT` | Same as listen port | The port to advertise to trackers and DHT. If not set, same as `--listen-port`. Useful when behind NAT with port forwarding to a different port. |
| `--listen-ip <IP>` | — | `RQBIT_LISTEN_IP` | `::` | IP address to listen on. Default `::` listens on all interfaces for both IPv4 and IPv6. Use `0.0.0.0` for IPv4 only on all interfaces, or a specific IP to bind to one interface. |
| `--disable-tcp-listen` | — | `RQBIT_TCP_LISTEN_DISABLE` | `false` | Disable listening for incoming connections over TCP. Outgoing TCP connections can still be made (see `--disable-tcp-connect`). |
| `--disable-tcp-connect` | — | `RQBIT_TCP_CONNECT_DISABLE` | `false` | Disable outgoing connections over TCP. Listening for incoming TCP connections is still enabled by default (see `--disable-tcp-listen`). |
| `--experimental-enable-utp-listen` | — | `RQBIT_EXPERIMENTAL_UTP_LISTEN_ENABLE` | `false` | Enable listening and connecting over uTP (Micro Transport Protocol). This is an experimental feature. |
| `--ipv4-only` | — | `RQBIT_IPV4_ONLY` | `false` | Force IPv4 only. Disables IPv6 for all networking. |
| `--bind-device <NAME>` | — | `RQBIT_BIND_DEVICE` | — | Bind all network sockets (DHT, BT-UDP, BT-TCP, trackers, LSD) to a specific network device. On macOS uses `IP(V6)_BOUND_IF`, on Linux uses `SO_BINDTODEVICE`. **Not supported on Windows** (will error). |

#### Listener Mode Logic

The combination of `--disable-tcp-listen` and `--experimental-enable-utp-listen` determines the listener mode:

| `--disable-tcp-listen` | `--experimental-enable-utp-listen` | Result |
|---|---|---|
| `false` (default) | `false` (default) | TCP only |
| `false` | `true` | TCP and uTP |
| `true` | `false` | **No listener** (cannot use `share`) |
| `true` | `true` | uTP only |

### UPnP

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--disable-upnp-port-forward` | — | `RQBIT_UPNP_PORT_FORWARD_DISABLE` | `false` | Disable UPnP port forwarding on your router. By default, rqbit will try to publish the listen port through UPnP. |
| `--enable-upnp-server` | — | `RQBIT_UPNP_SERVER_ENABLE` | `false` | Run a UPnP/DLNA Media Server on the HTTP API listen address, making downloaded media files discoverable by smart TVs and media players on your LAN. Requires `--http-api-listen-addr` to be a non-loopback address (e.g., `0.0.0.0:3030`). |
| `--upnp-server-friendly-name <NAME>` | — | `RQBIT_UPNP_SERVER_FRIENDLY_NAME` | `rqbit@<hostname>` | The display name of the UPnP server as shown on network devices. |

### Rate Limiting

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--ratelimit-download <BPS>` | — | `RQBIT_RATELIMIT_DOWNLOAD` | Unlimited | Limit download speed in bytes per second. Value must be a positive integer (e.g., `1048576` for 1 MiB/s). |
| `--ratelimit-upload <BPS>` | — | `RQBIT_RATELIMIT_UPLOAD` | Unlimited | Limit upload speed in bytes per second. Value must be a positive integer (e.g., `524288` for 512 KiB/s). |

### Blocklist / Allowlist

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--blocklist-url <URL>` | — | `RQBIT_BLOCKLIST_URL` | — | Download a P2P blocklist from this URL and block connections from/to those peers. Supports `file:///`, `http://`, and `https://` URLs. Format: newline-delimited `name:start_ip-end_ip`. Supports `.gz` compressed files. Example: `https://github.com/Naunter/BT_BlockLists/raw/refs/heads/master/bt_blocklists.gz` |
| `--allowlist-url <URL>` | — | `RQBIT_ALLOWLIST_URL` | — | Download a P2P allowlist from this URL and block **ALL** connections except from/to those peers (whitelist mode). Same format and URL support as `--blocklist-url`. |

### Trackers

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--tracker-refresh-interval <DURATION>` | `-i` | `RQBIT_TRACKER_REFRESH_INTERVAL` | Tracker-specified | Force a specific tracker polling interval. Trackers normally send their preferred refresh interval (often ~30 minutes). This option overrides that value. Accepts duration strings (e.g., `30s`, `5m`). |
| `--trackers-filename <PATH>` | — | `RQBIT_TRACKERS_FILENAME` | — | Path to a file containing tracker URLs (one per line). These trackers will always be used for each torrent in addition to any trackers embedded in the torrent file/magnet link. |
| `--disable-trackers` | — | `RQBIT_TRACKERS_DISABLE` | `false` | Disable trackers entirely. Useful for debugging DHT, LSD, and `--initial-peers` functionality. |
| `--disable-lsd` | — | `RQBIT_LSD_DISABLE` | `false` | Disable Local Service Discovery (LSD). By default, rqbit announces torrents on the LAN to find local peers. |

### Storage

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--experimental-mmap-storage` | — | — | `false` | Use memory-mapped (mmap) file-backed storage. Any advantages are questionable and unproven. Use only if you know what you are doing. |
| `--concurrent-init-limit <N>` | — | `RQBIT_CONCURRENT_INIT_LIMIT` | `5` | How many torrents can be initializing (rehashing) at the same time. Higher values may speed up startup but increase I/O and CPU load. |

### Proxy

| Option | Short | Env Var | Default | Description |
|---|---|---|---|---|
| `--socks-url <URL>` | — | `RQBIT_SOCKS_PROXY_URL` | — | Use a SOCKS5 proxy for all outgoing connections. Format: `socks5://[username:password@]host:port`. You may also want to disable incoming connections via `--disable-tcp-listen` when using a proxy. |

### Miscellaneous

| Option | Short | Env Var | Default | Platform |
|---|---|---|---|---|
| `--umask <OCTAL>` | — | `RQBIT_UMASK` | Inherited from environment (usually `022`) | **Linux/macOS only** |

> The `--umask` option sets the process umask to control the file mode of created files. Must be a 3-digit octal value (e.g., `022`, `077`). See [umask(2)](https://man7.org/linux/man-pages/man2/umask.2.html) for details.

---

## Environment-Only Variables

These variables are set via environment only and do **not** have corresponding CLI flags:

| Variable | Description |
|---|---|
| `RQBIT_HTTP_BASIC_AUTH_USERPASS` | Enable HTTP Basic Authentication for the API. Format: `username:password`. |

---

## Compile-Time Features

These features are controlled at **build time** and are not available as runtime options. They alter the available CLI options and runtime behavior.

| Feature | Default | Description |
|---|---|---|
| `default-tls` | ✅ | Use the platform's default TLS implementation. |
| `rust-tls` | ❌ | Use rustls (pure Rust TLS). Mutually exclusive with `default-tls`. |
| `openssl-vendored` | ❌ | Build and statically link OpenSSL. |
| `postgres` | ✅ | Enable PostgreSQL as a session persistence backend. Required for `postgres://` in `--persistence-location`. |
| `webui` | ✅ | Embed the web UI into the HTTP API server. |
| `prometheus` | ✅ | Enable Prometheus metrics export at `/metrics` endpoint. |
| `disable-upload` | ❌ | Adds the `--disable-upload` / `RQBIT_DISABLE_UPLOAD` CLI flag. When enabled, rqbit won't share piece availability and will disconnect on download requests. Useful if upload bandwidth interferes with other Internet usage. |
| `debug_slow_disk` | ❌ | Enable slow-disk simulation middleware for development/testing. |
| `tokio-console` | ❌ | Enable tokio-console integration for async runtime debugging. |
| `timed_existence` | ❌ | Enable timed existence tracking. |
| `_disable_disk_write_net_benchmark` | ❌ | Disable disk writes for network benchmarking purposes. |

### Feature-Gated CLI Flags

| CLI Flag | Required Feature | Env Var | Description |
|---|---|---|---|
| `--disable-upload` | `disable-upload` | `RQBIT_DISABLE_UPLOAD` | Disable uploading entirely. Won't share piece availability and will disconnect on download requests. |

---

## Usage Examples

### Start a Server

```bash
# Basic server with default settings
rqbit server start /path/to/downloads

# Server with custom HTTP API address, persistence, and watch folder
rqbit --http-api-listen-addr 0.0.0.0:3030 \
      server start \
      --persistence-location /var/lib/rqbit \
      --watch-folder /path/to/watch \
      /path/to/downloads

# Server with UPnP media server for DLNA
rqbit --http-api-listen-addr 0.0.0.0:3030 \
      --enable-upnp-server \
      --upnp-server-friendly-name "My rqbit Server" \
      server start /path/to/downloads

# Server with PostgreSQL persistence
rqbit server start \
      --persistence-location "postgres://user:pass@localhost/rqbit" \
      /path/to/downloads

# Server with rate limiting and blocklist
rqbit --ratelimit-download 5242880 \
      --ratelimit-upload 1048576 \
      --blocklist-url "https://example.com/blocklist.gz" \
      server start /path/to/downloads

# Server behind SOCKS5 proxy
rqbit --socks-url socks5://user:pass@proxy.example.com:1080 \
      --disable-tcp-listen \
      server start /path/to/downloads
```

### Download Torrents

```bash
# Download a torrent to current directory
rqbit download "magnet:?xt=urn:btih:..."

# Download to a specific folder with file filtering
rqbit download -o /path/to/output -r "\.mkv$" /path/to/file.torrent

# Download and exit when finished
rqbit download -e -o /path/to/output "magnet:?xt=urn:btih:..."

# List torrent contents without downloading
rqbit download --list "magnet:?xt=urn:btih:..."

# Download multiple torrents at once
rqbit download -o /downloads \
      /path/to/file1.torrent \
      "magnet:?xt=urn:btih:abc123" \
      "https://example.com/file2.torrent"

# Download with initial peers and no HTTP API
rqbit download --initial-peers 192.168.1.10:6881,10.0.0.5:51413 \
      --disable-http-api \
      /path/to/file.torrent

# Download with verbose logging
rqbit -v debug download -o /downloads "magnet:?xt=urn:btih:..."
```

### Share Files

```bash
# Share a file with tracker announcement
rqbit share /path/to/file.iso udp://tracker.example.com:1337/announce

# Share a directory with custom name
rqbit share -n "My Collection" /path/to/directory

# Share with multiple trackers
rqbit share /path/to/file.iso \
      udp://tracker1.example.com:1337/announce,udp://tracker2.example.com:1337/announce
```

### Environment Variable Configuration

```bash
# Configure everything via environment variables
export RQBIT_LOG_LEVEL_CONSOLE=debug
export RQBIT_HTTP_API_LISTEN_ADDR=0.0.0.0:3030
export RQBIT_LISTEN_PORT=4240
export RQBIT_PEER_CONNECT_TIMEOUT=5s
export RQBIT_PEER_READ_WRITE_TIMEOUT=120s
export RQBIT_CONCURRENT_INIT_LIMIT=3
export RQBIT_HTTP_BASIC_AUTH_USERPASS=admin:secretpassword
export RQBIT_LOG_FILE=/var/log/rqbit.log

rqbit server start /path/to/downloads
```

---

## Default Behavior Notes

### HTTP API Listen Address Defaults (per subcommand)

| Subcommand | Default Address | Read-Only |
|---|---|---|
| `server start` | `127.0.0.1:3030` | No (read-write) |
| `download` | `127.0.0.1:<ephemeral>` | Yes |
| `share` | `[::]:0` (all interfaces, ephemeral) | Yes |

### Listen Port Defaults (for peer connections)

| Subcommand | Default Port |
|---|---|
| `server start` | `4240` |
| `download` | Ephemeral (random available port) |
| `share` | Ephemeral (random available port) |

### Persistence Defaults

| Subcommand | Session Persistence | DHT Persistence |
|---|---|---|
| `server start` | ✅ Enabled (JSON, OS-specific dir) | ✅ Enabled |
| `download` | ❌ Disabled | ❌ Disabled |
| `share` | ❌ Disabled | ❌ Disabled |

### UPnP Port Forwarding Defaults

| Subcommand | UPnP Port Forward |
|---|---|
| `server start` | ✅ Enabled (unless `--disable-upnp-port-forward`) |
| `download` | ❌ Always disabled |
| `share` | ✅ Enabled (unless `--disable-upnp-port-forward`) |

### Systemd Socket Activation (Linux only)

When rqbit is started under systemd with socket activation, the systemd-provided socket takes precedence over `--http-api-listen-addr`. The environment variables `LISTEN_PID` and `LISTEN_FDS` are used for detection. Exactly **1 socket** must be passed by systemd.

### Graceful Shutdown (Linux/macOS)

On receiving `SIGINT` or `SIGTERM`:
1. First signal triggers a graceful shutdown attempt.
2. Second signal forces immediate shutdown via `exit(1)`.
3. If graceful shutdown takes longer than **5 seconds**, the process is force-killed.

On Windows, graceful shutdown is handled by the tokio runtime's default signal handling.

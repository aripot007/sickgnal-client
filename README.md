# Sickgnal

Sickgnal is an end-to-end encrypted chat project built for a school project.  
This repository contains both reusable chat libraries and frontend applications (GUI + TUI).

Server repository: [BaptTF/sickgnal-server](https://github.com/BaptTF/sickgnal-server)

## Project structure

- `lib/sickgnal_core`: core chat + E2E protocol logic.
- `lib/sickgnal_sdk`: high-level SDK used by frontends.
- `lib/sickgnal_insecure_tls`: custom/experimental TLS implementation.
- `app/sickgnal_gui`: Slint-based desktop GUI client.
- `app/sickgnal_tui`: terminal (Ratatui) client.
- `app/sickgnal_test`: end-to-end test runner against a running server.
- `app/test_tls`: small TLS test CLI.

## Prerequisites

- Rust toolchain (edition 2024 crates).
- A running `sickgnal-server` instance (local or remote).

Optional (Nix users): a dev shell is available via `flake.nix`.

## Build

From the repository root:

```bash
cargo build
```

## Run

### GUI client

```bash
cargo run -p sickgnal_gui -- --server 127.0.0.1:8080 --tls none
```

Default values:

- `--data-dir ./storage`
- `--server localhost:8080`
- `--tls rustls`

### TUI client

```bash
cargo run -p sickgnal_tui -- --server 127.0.0.1:8080 --tls none
```

Default values:

- `--data-dir ./storage`
- `--server localhost:8080`
- `--tls rustls`

### End-to-end local test runner

`sickgnal_test` expects a server binary path in `SICKGNAL_SERVER_BIN` (defaults to `/tmp/sickgnal-server`).

```bash
SICKGNAL_SERVER_BIN=/path/to/sickgnal-server cargo run -p sickgnal_test
```

### TLS test utility

```bash
cargo run -p test_tls -- --ca-file app/test_tls/server/ca_cert.pem localhost 4267
```

## CLI arguments

### `sickgnal_gui`

- `--data-dir <PATH>`: directory for account/profile storage.
- `--server <HOST[:PORT]>`: server address.
- `--tls <rustls|insecure|none>`: TLS backend.
- `--tls-ca <PATH>`: custom PEM CA certificate.

### `sickgnal_tui`

- `--data-dir <PATH>`: directory for account/profile storage.
- `--log <PATH>`: enable tracing and write logs to file.
- `--server <HOST:PORT>`: server address.
- `--tls <rustls|insecure|none>`: TLS backend.
- `--tls-ca <PATH>`: custom PEM CA certificate.

### `test_tls`

- `-c, --ca-file <PATH>`: custom root CA certificate.
- `<host>` (positional, default `localhost`): server host.
- `<port>` (positional, default `4267`): server port.

## TLS modes

- `rustls`: recommended for normal use.
- `insecure`: custom TLS implementation, made as part of the school project.
- `none`: plain TCP (development only).

## Notes

- This repository contains client-side libraries and frontends.
- The backend server is a separate repository: [https://github.com/BaptTF/sickgnal-server](https://github.com/BaptTF/sickgnal-server).

# rulay

`rulay` runs in one of two modes:

- `transmitter` — accepts connections from clients and receivers, verifies REALITY auth, proxies traffic
- `receiver` — connects to the transmitter and bridges it to the target upstream

## Parameters

Both modes accept:

- `--upstream-server`
- `--upstream-port`
- `--downstream-server`
- `--downstream-port`

Transmitter mode also accepts:

- `--server-priv` — base64url (no-pad) encoded 32-byte X25519 server private key for REALITY auth verification
- `--redirect-server` — `host:port` to redirect non-REALITY clients to (e.g. a cover website)

If any parameter is omitted, mode-specific defaults are used.

## Local Run

Build:

```bash
cargo build --release
```

Run `transmitter`:

```bash
cargo run -- \
  --mode transmitter \
  --upstream-server 0.0.0.0 \
  --upstream-port 8444 \
  --downstream-server 0.0.0.0 \
  --downstream-port 443 \
  --server-priv 12345ABCD \
  --redirect-server example.com:443
```

Run `receiver`:

```bash
cargo run -- \
  --mode receiver \
  --upstream-server 0.0.0.0 \
  --upstream-port 8443 \
  --downstream-server 0.0.0.0 \
  --downstream-port 8444
```

## Cross-compilation (macOS → Linux x86_64)

The Docker image targets `linux/amd64`. To build the binary on macOS without Docker:

```bash
brew install zig
cargo install cargo-zigbuild
rustup target add x86_64-unknown-linux-gnu
cargo zigbuild --release --target x86_64-unknown-linux-gnu
```

The binary is placed at `target/x86_64-unknown-linux-gnu/release/rulay` and is picked up by the Dockerfile directly.

## Docker Build

Build the image (uses the pre-compiled binary from `target/x86_64-unknown-linux-gnu/release/rulay`):

```bash
./install.sh --build-only
```

Or directly:

```bash
docker build -t rulay .
```

## Docker Run via install.sh

Run `transmitter`:

```bash
./install.sh \
  --mode transmitter \
  --upstream-server 0.0.0.0 \
  --upstream-port 8554 \
  --downstream-server 0.0.0.0 \
  --downstream-port 443 \
  --server-priv ABCD123123EF \
  --redirect-server example:443
```

Run `receiver`:

```bash
./install.sh \
  --mode receiver \
  --upstream-server  host.docker.internal  \
  --upstream-port 8553 \
  --downstream-server 0.0.0.0 \
  --downstream-port 8554
```

Additional options:

- `--image-name <name>` — Docker image tag (default: `rulay`)
- `--container-name <name>` — container name (default: `rulay`)
- `--build-only` — build the image and exit without starting a container

`install.sh` publishes both configured ports from the container to the host using the same port numbers.

## Tokio Console (profiling)

The binary includes [tokio-console](https://github.com/tokio-rs/console) instrumentation. The console gRPC server listens on port `6669` inside the container (`install.sh` publishes it automatically).

Install the CLI:

```bash
cargo install tokio-console
```

Connect to a remote server via SSH tunnel:

```bash
ssh -L 6669:localhost:6669 root@<server-ip>
```

Then in another terminal:

```bash
tokio-console
```

This opens an interactive TUI showing all named async tasks, their poll times, waker counts, etc.

Named tasks:

| Name | Location |
|---|---|
| `upstream-listener` | accepts receiver connections |
| `ping-loop` | pings receiver connections in the pool |
| `downstream-client` | handles an incoming client |
| `copy-bidir-client` | bidirectional copy client <-> receiver |
| `copy-bidir-redirect` | bidirectional copy for redirected (non-REALITY) clients |
| `receiver-connect` | establishes connection to transmitter |
| `receiver-ping-loop` | responds to pings / waits for data |

## Direct docker run

```bash
docker run --rm \
  --add-host host.docker.internal:host-gateway \
  -e MODE=transmitter \
  -e UPSTREAM_SERVER=0.0.0.0 \
  -e UPSTREAM_PORT=8444 \
  -e DOWNSTREAM_SERVER=0.0.0.0 \
  -e DOWNSTREAM_PORT=443 \
  -e SERVER_PRIV=ABCD123123EF \
  -e REDIRECT_SERVER=example:443 \
  -p 8444:8444 \
  -p 443:443 \
  rulay
```

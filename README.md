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

## Running tests

Tests for zero-copy plumbing (`src/utils/zerocopy_async.rs`) are Linux-only — they use `splice` / `epoll`, which are unavailable on macOS.

### On Linux

```bash
cargo test
```

Run a single test:

```bash
cargo test test_register_socket_pair -- --nocapture
```

### On macOS (via Lima)

Use the same Lima VM configured in "Remote debug" below:

```bash
limactl start default          # first time only
lima cargo test                # runs tests inside the VM against the repo
```

Use a dedicated target dir to avoid artefact collisions with the macOS build:

```bash
lima env CARGO_TARGET_DIR=target/lima cargo test
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

## Remote debug (macOS → Linux over Lima)

Linux-specific code (`splice`, `epoll`, etc.) cannot be tested or debugged natively on macOS. This section sets up a reproducible remote-debug flow: tests run under `gdbserver` inside a Lima VM, RustRover attaches via a direct SSH tunnel.

On an Apple Silicon host Lima defaults to `aarch64` guests. For an `x86_64` guest, create a separate Lima instance with `limactl create --arch x86_64 --name linux-x64 template://default` and substitute `default` with `linux-x64` in the commands below.

### One-time setup

**1. Install tooling on the host:**

```bash
brew install lima jq
```

**2. Start the Lima VM:**

```bash
limactl start default
```

**3. Install `gdbserver` inside the VM:**

```bash
lima sudo apt-get update && lima sudo apt-get install -y gdbserver
```

**4. Do NOT use `portForwards` in `~/.lima/default/lima.yaml` for the debug port.** Lima's built-in forwarder mangles the gdb-remote binary protocol (inserts `timeout` markers into the stream, which GDB rejects as `Remote replied unexpectedly to 'vMustReplyEmpty': timeout`). Use a plain SSH tunnel instead (see step 6 of the per-session flow).

**5. Create a RustRover run configuration** (requires RustRover 2025.2+).

Run → Edit Configurations → `+` → **Remote Debug**:

| Field | Value |
|---|---|
| Name | `rulay-remote` |
| Debugger | `Bundled LLDB` (e.g. `LLDB 19`) — connects to `gdbserver` via the gdb-remote protocol. `Bundled GDB` is not always exposed in the dropdown on macOS; if needed, use `Custom GDB` pointing to `/Applications/RustRover.app/Contents/bin/gdb/mac/aarch64/bin/gdb`. |
| `'target remote' args` | `localhost:1234` |
| Symbol file | *(filled in per session — see step 4 below)* |
| Sysroot | *(leave empty; set to `target:` only if the debugger complains about libc symbols)* |

### Per-session flow

**1. Start the SSH tunnel** on the host (survives rebuilds; only needs to be restarted if the VM restarts):

```bash
ssh -F ~/.lima/default/ssh.config -fN -L 1234:localhost:1234 lima-default
```

Verify it is listening:

```bash
lsof -iTCP:1234 -sTCP:LISTEN
```

**2. Enter the VM shell:**

```bash
lima
```

**3. Build the test binary and launch `gdbserver`** using the helper script in the repo root:

```bash
./debug-test-linux.sh <test_name> [port]
```

Example:

```bash
./debug-test-linux.sh register_socket_pair
```

The script compiles tests into `target/lima/` (kept separate from the macOS target dir to avoid artefact collisions), prints the binary path, and execs `gdbserver 0.0.0.0:<port>`. Expected output:

```
binary: /Users/dmitrii/rust/rulay/target/lima/debug/deps/rulay-<hash>
listening: 0.0.0.0:1234
Process ... created; pid = ...
Listening on port 1234
```

**4. Copy the printed `binary:` path** into the **Symbol file** field of the `rulay-remote` configuration in RustRover. The hash in the file name changes whenever the test crate is rebuilt, so this field needs updating after meaningful source changes.

**5. Set breakpoints** in `src/utils/zerocopy_async.rs` (or elsewhere) and press **Debug** on the `rulay-remote` configuration. RustRover connects to the SSH-tunnelled `localhost:1234`, loads symbols, and stops at the first breakpoint.

**6. Stop the session** by pressing `Ctrl+C` in the Lima shell where `gdbserver` is running. The test binary exits; the SSH tunnel stays up for the next iteration.

### Troubleshooting

- **`Cannot run debugger`** — RustRover version older than 2025.2 (the Remote Debug template did not exist before that). Update via Toolbox.
- **`Remote replied unexpectedly to 'vMustReplyEmpty': timeout`** — traffic is going through Lima's `portForwards` or another proxy. Verify the listener on port 1234 is `ssh`, not `limactl`: `lsof -iTCP:1234 -sTCP:LISTEN`.
- **`Bundled GDB` missing from the Debugger dropdown** — known RustRover quirk on macOS. Use `Bundled LLDB` (it speaks gdb-remote) or `Custom GDB` pointing to `/Applications/RustRover.app/Contents/bin/gdb/mac/aarch64/bin/gdb`.
- **Architecture mismatch** — check that the Lima VM arch matches what `cargo` is building. `uname -m` inside the VM and `file target/lima/debug/deps/rulay-*` on the host must agree.
- **Unresolved breakpoints** — usually a stale `Symbol file` path after a rebuild. Re-run the helper script and update the config.

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

окей, у нас есть функция bidirectional_splice(), которая           
блокируется на epoll_wait(). хочу epoll_wait вытащить в отдельный  
тред, чтобы обслуживать epoll, и каким то образом уведомлять       
остальные подписавшиеся bidirectional_splice() о том, что пришло   
обновление и нужно запустить splice(). давай составим план, как    
это можно сделать и положим в task.md  
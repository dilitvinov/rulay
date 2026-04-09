# rulay

`rulay` runs in one of two modes:

- `transmitter`
- `receiver`

Both modes accept runtime connection parameters as separate server and port values:

- `--upstream-server`
- `--upstream-port`
- `--downstream-server`
- `--downstream-port`

Transmitter mode also accepts:

- `--server-priv` — base64url (no-pad) encoded 32-byte X25519 server private key for REALITY auth verification

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
  --downstream-port 8443 \
  --server-priv uM5Zol5nBgyqDrn2RYGhmTeoONiULxeLMhkeDqMtMUE
```

Run `receiver`:

```bash
cargo run -- \
  --mode receiver \
  --upstream-server 94.177.170.43 \
  --upstream-port 8443 \
  --downstream-server 0.0.0.0 \
  --downstream-port 8444
```

If any of these parameters are omitted, mode-specific defaults are used.

## Docker Build

Build only:

```bash
./install.sh --build-only
```

This creates the Docker image `rulay` by default.

## Docker Run Via install.sh

Run `transmitter`:

```bash
./install.sh \
  --mode transmitter \
  --upstream-server 0.0.0.0 \
  --upstream-port 8554 \
  --downstream-server 0.0.0.0 \
  --downstream-port 443 \
  --server-priv uM5Zol5nBgyqDrn2RYGhmTeoONiULxeLMhkeDqMtMUE \
  --redirect-server strm-mar-190.strm.yandex.net:443
```

Run `receiver`:

```bash
./install.sh \
  --mode receiver \
  --upstream-server  host.docker.internal  \
  --upstream-port 8553 \
  --downstream-server 194.87.236.129 \
  --downstream-port 8554
```

Additional options:

- `--image-name <name>`
- `--container-name <name>`
- `--build-only`

`install.sh` publishes both configured ports from the container to the host using the same port numbers.

Examples:

- `--upstream-port 9001` results in `-p 9001:9001`
- `--downstream-port 9002` results in `-p 9002:9002`

If a port is not passed to `install.sh`, the script uses the mode default and publishes that default port.

## Direct docker run

You can also run the image directly:

```bash
docker build -t rulay .
```

```bash
docker run --rm \
  -e MODE=transmitter \
  -e UPSTREAM_SERVER=0.0.0.0 \
  -e UPSTREAM_PORT=8444 \
  -e DOWNSTREAM_SERVER=0.0.0.0 \
  -e DOWNSTREAM_PORT=8443 \
  -e SERVER_PRIV=uM5Zol5nBgyqDrn2RYGhmTeoONiULxeLMhkeDqMtMUE \
  -p 8444:8444 \
  -p 8443:8443 \
  rulay
```

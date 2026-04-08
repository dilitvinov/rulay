# VLESS + REALITY in Docker

This bundle deploys `Xray-core` with `VLESS TCP + REALITY` directly on a Ubuntu or Debian server, using your existing `config.json`.

## What it does

- installs `docker`, `docker compose`, `openssl`, and `ufw` when missing;
- pulls the official `ghcr.io/xtls/xray-core:latest` image;
- reads the inbound port and Reality parameters from `config.json`;
- derives the public key from the private key already present in `config.json`;
- opens the selected TCP port in `ufw`;
- starts the container and prints a ready client URI.

## Requirements

- a server with Ubuntu or Debian;
- shell access on that server with a user that can run `apt-get`, `ufw`, and Docker commands.

If port `443` is already occupied by Nginx, Caddy, or another service, set `XRAY_PORT` to a different value before deploy.

## Quick start

Copy the folder to the server and run there:

```bash
cd /path/to/infra/vless-reality
sudo SERVER_HOST=203.0.113.10 ./deploy-local.sh
```

If you omit `SERVER_HOST`, the script tries to detect the first server IP via `hostname -I`.

## Optional variables

```bash
XRAY_PORT=443
DEPLOY_DIR=/opt/xray-reality
CLIENT_NAME=my-reality-node
```

Normally `XRAY_PORT`, `REALITY_SERVER_NAME`, `REALITY_DEST`, `UUID`, and `SHORT_ID` are taken from `config.json`. Override them only if you know why you need that.

## After deploy

Check the service on the server:

```bash
docker compose -f /opt/xray-reality/docker-compose.yml ps
docker logs xray-reality --tail 100
ufw status
ss -ltnp | grep ':443'
```

The script prints a `vless://` URI you can import into a client such as `v2rayN`, `v2rayNG`, `Streisand`, `Hiddify`, or `sing-box`.

## Notes

- The script uses the official Xray image and current example structure from `XTLS/Xray-core` and `XTLS/Xray-examples`.
- This setup assumes a dedicated inbound port for Xray and does not multiplex with an existing HTTPS site.

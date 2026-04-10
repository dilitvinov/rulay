# VLESS Test Client

Docker container with a VLESS client (sing-box) and curl for local testing.

sing-box connects to the transmitter via VLESS Reality and exposes a SOCKS5 proxy on port 1080.
curl sends requests through the proxy.

## Prerequisites

Transmitter must be running on the host machine on port 443.

## Quick test

```bash
./test.sh
```

Builds the container, sends a request to `https://ifconfig.me` through the VLESS proxy, and stops.
If the output shows the VPS IP (not your local IP) — the proxy works.

## Interactive usage

```bash
docker compose up -d --build
docker exec -it vless-test-client sh
```

Inside the container:

```bash
curl --proxy socks5h://127.0.0.1:1080 https://ifconfig.me
curl --proxy socks5h://127.0.0.1:1080 https://example.com
```

Stop:

```bash
docker compose down
```

## Configuration

Edit `config.json` to change the VLESS connection parameters (server, uuid, public_key, etc.).
The config is mounted as a volume — no rebuild needed after changes, just restart:

```bash
docker compose restart
```

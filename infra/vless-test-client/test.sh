#!/bin/sh
set -eu

cd "$(dirname "$0")"

echo "Building and starting vless-test-client..."
docker compose up -d --build

echo ""
echo "Running test request through VLESS proxy..."
docker exec vless-test-client \
    curl -sf --proxy socks5h://127.0.0.1:1080 --max-time 10 https://ifconfig.me

echo ""
echo ""
echo "Test passed. Stopping..."
docker compose down

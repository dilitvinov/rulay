#!/bin/sh
set -e

# Start sing-box in background
sing-box run -c /etc/sing-box/config.json &
SINGBOX_PID=$!

# Wait for SOCKS5 proxy to be ready
for i in $(seq 1 30); do
    if nc -z 127.0.0.1 1080 2>/dev/null; then
        echo "sing-box is ready"
        break
    fi
    sleep 0.2
done

# If arguments passed — run them through the proxy, then exit
if [ "$#" -gt 0 ]; then
    exec "$@"
fi

# Otherwise keep running for interactive use
echo "SOCKS5 proxy is running on 127.0.0.1:1080"
echo "Usage: curl --proxy socks5h://127.0.0.1:1080 https://example.com"
wait $SINGBOX_PID

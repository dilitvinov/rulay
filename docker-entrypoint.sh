#!/bin/sh
set -eu

if [ "$#" -gt 0 ]; then
    exec /usr/local/bin/rulay "$@"
fi

if [ -z "${MODE:-}" ]; then
    echo "MODE is required when no explicit command arguments are passed." >&2
    echo "Set MODE=receiver|transmitter or pass CLI args directly." >&2
    exit 1
fi

set -- --mode "$MODE"

if [ -n "${UPSTREAM_SERVER:-}" ]; then
    set -- "$@" --upstream-server "$UPSTREAM_SERVER"
fi

if [ -n "${UPSTREAM_PORT:-}" ]; then
    set -- "$@" --upstream-port "$UPSTREAM_PORT"
fi

if [ -n "${DOWNSTREAM_SERVER:-}" ]; then
    set -- "$@" --downstream-server "$DOWNSTREAM_SERVER"
fi

if [ -n "${DOWNSTREAM_PORT:-}" ]; then
    set -- "$@" --downstream-port "$DOWNSTREAM_PORT"
fi

if [ -n "${SERVER_PRIV:-}" ]; then
    set -- "$@" --server-priv "$SERVER_PRIV"
fi

exec /usr/local/bin/rulay "$@"

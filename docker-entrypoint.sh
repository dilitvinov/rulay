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

if [ -n "${UPSTREAM_ADDR:-}" ]; then
    set -- "$@" --upstream-addr "$UPSTREAM_ADDR"
fi

if [ -n "${DOWNSTREAM_ADDR:-}" ]; then
    set -- "$@" --downstream-addr "$DOWNSTREAM_ADDR"
fi

exec /usr/local/bin/rulay "$@"

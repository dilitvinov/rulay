#!/bin/sh
set -eu

IMAGE_NAME="${IMAGE_NAME:-rulay}"
CONTAINER_NAME="${CONTAINER_NAME:-rulay}"
MODE=""
UPSTREAM_SERVER=""
UPSTREAM_PORT=""
DOWNSTREAM_SERVER=""
DOWNSTREAM_PORT=""
SERVER_PRIV=""
REDIRECT_SERVER=""
NO_RUN=0

default_upstream_port() {
    case "$1" in
        transmitter) echo "8444" ;;
        receiver) echo "8443" ;;
        *) return 1 ;;
    esac
}

default_downstream_port() {
    case "$1" in
        transmitter) echo "8443" ;;
        receiver) echo "8444" ;;
        *) return 1 ;;
    esac
}

usage() {
    cat <<'EOF'
Usage:
  ./install.sh --mode <receiver|transmitter> [--upstream-server host] [--upstream-port port] [--downstream-server host] [--downstream-port port] [--server-priv key] [--redirect-server host:port] [--image-name name] [--container-name name]
  ./install.sh --build-only [--image-name name]

Options:
  --mode              Runtime mode for the rulay binary.
  --upstream-server   Optional upstream host override.
  --upstream-port     Optional upstream port override.
  --downstream-server Optional downstream host override.
  --downstream-port   Optional downstream port override.
  --server-priv       Base64url X25519 private key for REALITY auth (transmitter).
  --redirect-server   Host:port to redirect non-REALITY clients to (transmitter).
  --image-name        Docker image tag. Default: rulay
  --container-name    Docker container name. Default: rulay
  --build-only        Build the image and exit without starting a container.
  --help              Show this help.

Environment overrides:
  IMAGE_NAME, CONTAINER_NAME
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --mode)
            MODE="$2"
            shift 2
            ;;
        --upstream-server)
            UPSTREAM_SERVER="$2"
            shift 2
            ;;
        --upstream-port)
            UPSTREAM_PORT="$2"
            shift 2
            ;;
        --downstream-server)
            DOWNSTREAM_SERVER="$2"
            shift 2
            ;;
        --downstream-port)
            DOWNSTREAM_PORT="$2"
            shift 2
            ;;
        --image-name)
            IMAGE_NAME="$2"
            shift 2
            ;;
        --container-name)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        --server-priv)
            SERVER_PRIV="$2"
            shift 2
            ;;
        --redirect-server)
            REDIRECT_SERVER="$2"
            shift 2
            ;;
        --build-only)
            NO_RUN=1
            shift
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

docker build -t "$IMAGE_NAME" -f Dockerfile .

if [ "$NO_RUN" -eq 1 ]; then
    exit 0
fi

if [ -z "$MODE" ]; then
    echo "--mode is required unless --build-only is used." >&2
    usage >&2
    exit 1
fi

docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true

if [ -z "$UPSTREAM_PORT" ]; then
    UPSTREAM_PORT="$(default_upstream_port "$MODE")"
fi

if [ -z "$DOWNSTREAM_PORT" ]; then
    DOWNSTREAM_PORT="$(default_downstream_port "$MODE")"
fi

set -- docker run -d \
    --name "$CONTAINER_NAME" \
    --add-host host.docker.internal:host-gateway \
    -e MODE="$MODE" \
    -e UPSTREAM_SERVER="$UPSTREAM_SERVER" \
    -e UPSTREAM_PORT="$UPSTREAM_PORT" \
    -e DOWNSTREAM_SERVER="$DOWNSTREAM_SERVER" \
    -e DOWNSTREAM_PORT="$DOWNSTREAM_PORT" \
    -e SERVER_PRIV="$SERVER_PRIV" \
    -e REDIRECT_SERVER="$REDIRECT_SERVER"

if [ "$MODE" = "transmitter" ]; then
    set -- "$@" \
        -p "$UPSTREAM_PORT:$UPSTREAM_PORT" \
        -p "$DOWNSTREAM_PORT:$DOWNSTREAM_PORT"
fi

set -- "$@" "$IMAGE_NAME"

"$@"

echo "Container started:"
echo "  image=$IMAGE_NAME"
echo "  container=$CONTAINER_NAME"
echo "  mode=$MODE"
if [ -n "$UPSTREAM_SERVER" ]; then
    echo "  upstream_server=$UPSTREAM_SERVER"
fi
echo "  upstream_port=$UPSTREAM_PORT"
if [ -n "$DOWNSTREAM_SERVER" ]; then
    echo "  downstream_server=$DOWNSTREAM_SERVER"
fi
echo "  downstream_port=$DOWNSTREAM_PORT"
if [ -n "$SERVER_PRIV" ]; then
    echo "  server_priv=***"
fi
if [ -n "$REDIRECT_SERVER" ]; then
    echo "  redirect_server=$REDIRECT_SERVER"
fi
if [ "$MODE" = "transmitter" ]; then
    echo "  published_ports=$UPSTREAM_PORT,$DOWNSTREAM_PORT"
fi

#!/bin/sh
set -eu

IMAGE_NAME="${IMAGE_NAME:-rulay}"
CONTAINER_NAME="${CONTAINER_NAME:-rulay}"
MODE=""
UPSTREAM_ADDR=""
DOWNSTREAM_ADDR=""
NO_RUN=0

usage() {
    cat <<'EOF'
Usage:
  ./install.sh --mode <receiver|transmitter> [--upstream-addr host:port] [--downstream-addr host:port] [--image-name name] [--container-name name]
  ./install.sh --build-only [--image-name name]

Options:
  --mode              Runtime mode for the rulay binary.
  --upstream-addr     Optional upstream address override.
  --downstream-addr   Optional downstream address override.
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
        --upstream-addr)
            UPSTREAM_ADDR="$2"
            shift 2
            ;;
        --downstream-addr)
            DOWNSTREAM_ADDR="$2"
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

docker run -d \
    --name "$CONTAINER_NAME" \
    -e MODE="$MODE" \
    -e UPSTREAM_ADDR="$UPSTREAM_ADDR" \
    -e DOWNSTREAM_ADDR="$DOWNSTREAM_ADDR" \
    "$IMAGE_NAME"

echo "Container started:"
echo "  image=$IMAGE_NAME"
echo "  container=$CONTAINER_NAME"
echo "  mode=$MODE"
if [ -n "$UPSTREAM_ADDR" ]; then
    echo "  upstream_addr=$UPSTREAM_ADDR"
fi
if [ -n "$DOWNSTREAM_ADDR" ]; then
    echo "  downstream_addr=$DOWNSTREAM_ADDR"
fi

#!/usr/bin/env bash

set -euo pipefail

SERVER_HOST="${SERVER_HOST:-}"
DEPLOY_DIR="${DEPLOY_DIR:-/opt/xray-reality}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_PATH="${SCRIPT_DIR}/config.json"

if [[ -z "${SERVER_HOST}" ]]; then
  SERVER_HOST="$(hostname -I 2>/dev/null | awk '{print $1}')"
fi

if [[ -z "${SERVER_HOST}" ]]; then
  echo "Set SERVER_HOST explicitly, for example: SERVER_HOST=203.0.113.10"
  exit 1
fi

if [[ ! -f "${CONFIG_PATH}" ]]; then
  echo "Config not found: ${CONFIG_PATH}"
  exit 1
fi

XRAY_PORT="${XRAY_PORT:-$(awk '
  /"inbounds"[[:space:]]*:/ { inbounds=1 }
  inbounds && /"port"[[:space:]]*:/ {
    gsub(/[^0-9]/, "", $0)
    print $0
    exit
  }
' "${CONFIG_PATH}")}"

UUID="${UUID:-$(awk '
  /"clients"[[:space:]]*:/ { clients=1 }
  clients && /"id"[[:space:]]*:/ {
    line=$0
    sub(/^.*"id"[[:space:]]*:[[:space:]]*"/, "", line)
    sub(/".*$/, "", line)
    print line
    exit
  }
' "${CONFIG_PATH}")}"

PRIVATE_KEY="${PRIVATE_KEY:-$(awk '
  /"privateKey"[[:space:]]*:/ {
    line=$0
    sub(/^.*"privateKey"[[:space:]]*:[[:space:]]*"/, "", line)
    sub(/".*$/, "", line)
    print line
    exit
  }
' "${CONFIG_PATH}")}"

REALITY_SERVER_NAME="${REALITY_SERVER_NAME:-$(awk '
  /"serverNames"[[:space:]]*:/ { server_names=1; next }
  server_names {
    line=$0
    gsub(/[",[:space:]]/, "", line)
    gsub(/\//, "", line)
    if (length(line) > 0 && line != "]") {
      print line
      exit
    }
  }
' "${CONFIG_PATH}")}"

REALITY_DEST="${REALITY_DEST:-$(awk '
  /"target"[[:space:]]*:/ || /"dest"[[:space:]]*:/ {
    line=$0
    sub(/^.*"(target|dest)"[[:space:]]*:[[:space:]]*"/, "", line)
    sub(/".*$/, "", line)
    print line
    exit
  }
' "${CONFIG_PATH}")}"

SHORT_ID="${SHORT_ID:-$(awk '
  /"shortIds"[[:space:]]*:/ { short_ids=1; next }
  short_ids {
    line=$0
    gsub(/[",[:space:]]/, "", line)
    if (length(line) > 0 && line != "]") {
      print line
      exit
    }
  }
' "${CONFIG_PATH}")}"

CLIENT_NAME="${CLIENT_NAME:-reality-${SERVER_HOST}}"

if [[ -z "${XRAY_PORT}" || -z "${UUID}" || -z "${PRIVATE_KEY}" || -z "${REALITY_SERVER_NAME}" || -z "${REALITY_DEST}" || -z "${SHORT_ID}" ]]; then
  echo "Failed to read required values from ${CONFIG_PATH}"
  exit 1
fi

apt_get_install() {
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y "$@"
}

if ! command -v docker >/dev/null 2>&1; then
  apt_get_install ca-certificates curl gnupg lsb-release docker.io docker-compose-plugin ufw
else
  apt_get_install ufw
fi

mkdir -p "${DEPLOY_DIR}"
cp "${SCRIPT_DIR}/docker-compose.yml" "${DEPLOY_DIR}/docker-compose.yml"
cp "${CONFIG_PATH}" "${DEPLOY_DIR}/config.json"

docker pull ghcr.io/xtls/xray-core:latest >/dev/null

KEYPAIR="$(docker run --rm ghcr.io/xtls/xray-core:latest x25519 -i "${PRIVATE_KEY}" 2>&1 || true)"
PUBLIC_KEY="$(printf '%s\n' "${KEYPAIR}" | awk '/Public key:|Password:/ { print $3; exit }')"

if [[ -z "${PUBLIC_KEY}" ]]; then
  echo "Failed to generate public key from private key"
  printf '%s\n' "${KEYPAIR}"
  exit 1
fi

cat > "${DEPLOY_DIR}/.env" <<ENV
XRAY_PORT=${XRAY_PORT}
ENV

ufw allow "${XRAY_PORT}/tcp"
ufw --force enable

cd "${DEPLOY_DIR}"
docker compose up -d

VLESS_URL="vless://${UUID}@${SERVER_HOST}:${XRAY_PORT}?type=tcp&security=reality&pbk=${PUBLIC_KEY}&fp=chrome&sni=${REALITY_SERVER_NAME}&sid=${SHORT_ID}&spx=%2F&flow=xtls-rprx-vision&encryption=none#${CLIENT_NAME}"

cat <<INFO
Deploy completed.

Server directory: ${DEPLOY_DIR}
Port: ${XRAY_PORT}
UUID: ${UUID}
Public key: ${PUBLIC_KEY}
Short ID: ${SHORT_ID}
SNI: ${REALITY_SERVER_NAME}
Target: ${REALITY_DEST}
Config: ${DEPLOY_DIR}/config.json

Client URI:
${VLESS_URL}

Checks:
  docker compose -f ${DEPLOY_DIR}/docker-compose.yml ps
  docker logs xray-reality --tail 100
  ufw status
INFO

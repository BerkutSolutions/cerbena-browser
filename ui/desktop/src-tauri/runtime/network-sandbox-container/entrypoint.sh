#!/usr/bin/env bash
set -Eeuo pipefail

PROXY_PORT="${CERBENA_PROXY_PORT:-17890}"
PROXY_LISTEN="${CERBENA_PROXY_LISTEN:-0.0.0.0:${PROXY_PORT}}"
RUNTIME_KIND="${CERBENA_RUNTIME_KIND:-amneziawg}"

cleanup() {
  if [[ "${RUNTIME_KIND}" == "amneziawg" && -n "${CONF_PATH:-}" ]]; then
    /usr/local/bin/awg-quick down "${CONF_PATH}" >/dev/null 2>&1 || true
  fi
  if [[ "${RUNTIME_KIND}" == "openvpn" && -n "${OPENVPN_PID:-}" ]]; then
    kill "${OPENVPN_PID}" >/dev/null 2>&1 || true
    wait "${OPENVPN_PID}" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM

case "${RUNTIME_KIND}" in
  amneziawg)
    CONFIG_PATH="${CERBENA_AMNEZIA_CONFIG:-/work/amnezia.conf}"
    WORK_DIR="/etc/cerbena-awg"
    CONF_PATH="${WORK_DIR}/awg0.conf"
    mkdir -p "${WORK_DIR}"
    cp "${CONFIG_PATH}" "${CONF_PATH}"
    chmod 600 "${CONF_PATH}"
    export WG_QUICK_USERSPACE_IMPLEMENTATION="${WG_QUICK_USERSPACE_IMPLEMENTATION:-/usr/local/bin/amneziawg-go}"
    /usr/local/bin/awg-quick up "${CONF_PATH}"
    exec /usr/local/bin/container-socks-proxy -listen "${PROXY_LISTEN}"
    ;;
  openvpn)
    CONFIG_PATH="${CERBENA_OPENVPN_CONFIG:-/work/openvpn.ovpn}"
    LOG_PATH="${CERBENA_ROUTE_LOG:-/work/route.log}"
    /usr/sbin/openvpn --config "${CONFIG_PATH}" --verb 3 --log "${LOG_PATH}" --suppress-timestamps &
    OPENVPN_PID=$!
    sleep 3
    exec /usr/local/bin/container-socks-proxy -listen "${PROXY_LISTEN}"
    ;;
  sing-box)
    CONFIG_PATH="${CERBENA_SINGBOX_CONFIG:-/work/sing-box.json}"
    exec /usr/local/bin/sing-box run -c "${CONFIG_PATH}"
    ;;
  *)
    echo "Unsupported CERBENA_RUNTIME_KIND=${RUNTIME_KIND}" >&2
    exit 1
    ;;
esac

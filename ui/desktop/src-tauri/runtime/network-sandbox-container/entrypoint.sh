#!/usr/bin/env bash
set -Eeuo pipefail

PROXY_PORT="${CERBENA_PROXY_PORT:-17890}"
PROXY_LISTEN="${CERBENA_PROXY_LISTEN:-0.0.0.0:${PROXY_PORT}}"
RUNTIME_KIND="${CERBENA_RUNTIME_KIND:-amneziawg}"

wait_for_openvpn_ready() {
  local log_path="$1"
  local timeout="${2:-30}"
  local deadline=$((SECONDS + timeout))
  while (( SECONDS < deadline )); do
    if [[ -f "${log_path}" ]] && grep -q "Initialization Sequence Completed" "${log_path}"; then
      return 0
    fi
    if [[ -n "${OPENVPN_PID:-}" ]] && ! kill -0 "${OPENVPN_PID}" >/dev/null 2>&1; then
      if [[ -f "${log_path}" ]]; then
        tail -n 50 "${log_path}" >&2 || true
      fi
      return 1
    fi
    sleep 1
  done
  if [[ -f "${log_path}" ]]; then
    tail -n 50 "${log_path}" >&2 || true
  fi
  return 1
}

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
    AUTH_PATH="${CERBENA_OPENVPN_AUTH:-}"
    /usr/sbin/openvpn --config "${CONFIG_PATH}" --verb 3 --log "${LOG_PATH}" --suppress-timestamps &
    OPENVPN_PID=$!
    if [[ -n "${AUTH_PATH}" && ! -f "${AUTH_PATH}" ]]; then
      echo "OpenVPN auth file not found: ${AUTH_PATH}" >&2
      exit 1
    fi
    if ! wait_for_openvpn_ready "${LOG_PATH}" 30; then
      echo "OpenVPN tunnel did not become ready" >&2
      exit 1
    fi
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

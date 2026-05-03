#!/usr/bin/env bash
set -Eeuo pipefail

RESOLV_CONF="/etc/resolv.conf"
BACKUP_PATH="/tmp/cerbena-resolv.conf.backup"

collect_foreign_options() {
  env | grep '^foreign_option_' | cut -d= -f2-
}

write_vpn_resolver() {
  local dns_servers=()
  local search_domains=()

  while IFS= read -r option; do
    case "${option}" in
      "dhcp-option DNS "*)
        dns_servers+=("${option#dhcp-option DNS }")
        ;;
      "dhcp-option DOMAIN-SEARCH "*)
        search_domains+=("${option#dhcp-option DOMAIN-SEARCH }")
        ;;
      "dhcp-option DOMAIN "*)
        search_domains+=("${option#dhcp-option DOMAIN }")
        ;;
    esac
  done < <(collect_foreign_options)

  if [[ ${#dns_servers[@]} -eq 0 && ${#search_domains[@]} -eq 0 ]]; then
    exit 0
  fi

  if [[ ! -f "${BACKUP_PATH}" && -f "${RESOLV_CONF}" ]]; then
    cp "${RESOLV_CONF}" "${BACKUP_PATH}"
  fi

  : > "${RESOLV_CONF}"
  if [[ ${#search_domains[@]} -gt 0 ]]; then
    printf 'search %s\n' "${search_domains[*]}" >> "${RESOLV_CONF}"
  fi
  for dns in "${dns_servers[@]}"; do
    printf 'nameserver %s\n' "${dns}" >> "${RESOLV_CONF}"
  done
  printf 'options ndots:1\n' >> "${RESOLV_CONF}"
}

restore_resolver() {
  if [[ -f "${BACKUP_PATH}" ]]; then
    cp "${BACKUP_PATH}" "${RESOLV_CONF}"
  fi
}

case "${script_type:-}" in
  up)
    write_vpn_resolver
    ;;
  down)
    restore_resolver
    ;;
esac

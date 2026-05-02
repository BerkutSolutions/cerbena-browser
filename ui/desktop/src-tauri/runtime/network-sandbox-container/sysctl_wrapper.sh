#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "-q" && "${2:-}" == "net.ipv4.conf.all.src_valid_mark=1" ]]; then
  exit 0
fi

exec /usr/sbin/sysctl "$@"

#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
status=0
podman compose down || status=$?
scripts/gvisor-controller.sh stop
exit "$status"

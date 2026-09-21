#!/usr/bin/env bash
set -euo pipefail

profile=${LIMA_INSTANCE:-locoder}
command -v limactl >/dev/null || { echo "missing required command: limactl" >&2; exit 1; }

if limactl list --quiet | grep -Fxq "$profile"; then
    limactl delete --force "$profile"
else
    echo "Lima instance $profile does not exist"
fi

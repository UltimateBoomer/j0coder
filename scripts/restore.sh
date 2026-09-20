#!/usr/bin/env bash
# Restore only into a fresh project. Existing volumes are refused, never overwritten.
set -euo pipefail
cd "$(dirname "$0")/.."
backup_dir=${1:?Usage: scripts/restore.sh ABSOLUTE_BACKUP_DIRECTORY}
[[ ! -e .env ]] || { echo 'Restore into a fresh checkout without .env' >&2; exit 1; }
for volume in postgres valkey; do
 if podman volume exists "practice_${volume}"; then echo "Existing volume practice_${volume}; refusing overwrite" >&2; exit 1; fi
 [[ -f "$backup_dir/${volume}.tar" ]]
done
umask 077
mkdir -p data
cp "$backup_dir/env" .env
cp -r "$backup_dir/config" data/config
for volume in postgres valkey; do
 podman volume create "practice_${volume}"
 podman volume import "practice_${volume}" "$backup_dir/${volume}.tar"
done
printf 'Restore complete. Review .env socket/origin, run preflight, then podman compose up -d.\n'

#!/usr/bin/env bash
# Restore only into a fresh project. Existing volumes are refused, never overwritten.
set -euo pipefail
cd "$(dirname "$0")/.."
usage() { echo 'Usage: scripts/restore.sh [--project-name NAME] ABSOLUTE_BACKUP_DIRECTORY' >&2; }
project_name=j0coder
backup_dir=
while [[ $# -gt 0 ]]; do
 case "$1" in
  --project-name) [[ $# -ge 2 ]] || { usage; exit 2; }; project_name=$2; shift 2 ;;
  -*) usage; exit 2 ;;
  *) [[ -z "$backup_dir" ]] || { usage; exit 2; }; backup_dir=$1; shift ;;
 esac
done
[[ -n "$backup_dir" ]] || { usage; exit 2; }
[[ "$backup_dir" = /* ]] || { echo 'Use an absolute backup path' >&2; exit 1; }
[[ "$project_name" =~ ^[a-z0-9][a-z0-9_-]*$ ]] || { echo 'Invalid Compose project name' >&2; exit 1; }
[[ ! -e .env ]] || { echo 'Restore into a fresh checkout without .env' >&2; exit 1; }
[[ ! -e data/config ]] || { echo 'Existing data/config; refusing overwrite' >&2; exit 1; }
[[ -f "$backup_dir/env" && -d "$backup_dir/config" ]] || { echo 'Missing backup credentials or configuration' >&2; exit 1; }
for volume in postgres valkey; do
 if podman volume exists "${project_name}_${volume}"; then
  echo "Existing volume ${project_name}_${volume}; refusing overwrite" >&2; exit 1
 else
  status=$?
  [[ "$status" == 1 ]] || { echo "Cannot inspect volume ${project_name}_${volume}" >&2; exit "$status"; }
 fi
 [[ -f "$backup_dir/${volume}.tar" ]] || { echo "Missing backup archive: $volume" >&2; exit 1; }
done
umask 077
mkdir -p data
cp "$backup_dir/env" .env
cp -r "$backup_dir/config" data/config
for volume in postgres valkey; do
 podman volume create "${project_name}_${volume}"
 podman volume import "${project_name}_${volume}" "$backup_dir/${volume}.tar"
done
printf 'Restore complete. Review .env socket/origin, then build the images, install and start gVisor, and run make up.\n'

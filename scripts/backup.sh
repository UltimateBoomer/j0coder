#!/usr/bin/env bash
# Cold, consistent backup of this Compose project only. Maintenance downtime is required.
set -euo pipefail
cd "$(dirname "$0")/.."
backup_dir=${1:?Usage: scripts/backup.sh ABSOLUTE_BACKUP_DIRECTORY}
[[ "$backup_dir" = /* ]] || { echo 'Use an absolute backup path' >&2; exit 1; }
mkdir -m 700 -p "$backup_dir"
trap 'podman compose start' EXIT
podman compose stop
for volume in postgres valkey; do
 podman volume export "practice_${volume}" --output "$backup_dir/${volume}.tar"
done
cp .env "$backup_dir/env"
cp -r data/config "$backup_dir/config"
cp Cargo.lock web/package-lock.json "$backup_dir/"
chmod -R go-rwx "$backup_dir"
printf 'Backup complete: %s\n' "$backup_dir"

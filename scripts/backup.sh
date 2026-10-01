#!/usr/bin/env bash
# Cold, consistent backup of this Compose project only. Maintenance downtime is required.
set -euo pipefail
cd "$(dirname "$0")/.."
usage() { echo 'Usage: scripts/backup.sh [--compose-file FILE] [--project-name NAME] [--leave-stopped] ABSOLUTE_BACKUP_DIRECTORY' >&2; }
compose_file=compose.yaml
project_name=j0coder
leave_stopped=false
backup_dir=
while [[ $# -gt 0 ]]; do
 case "$1" in
  --compose-file|--project-name)
   [[ $# -ge 2 ]] || { usage; exit 2; }
   if [[ "$1" == --compose-file ]]; then compose_file=$2; else project_name=$2; fi
   shift 2 ;;
  --leave-stopped) leave_stopped=true; shift ;;
  -*) usage; exit 2 ;;
  *) [[ -z "$backup_dir" ]] || { usage; exit 2; }; backup_dir=$1; shift ;;
 esac
done
[[ -n "$backup_dir" ]] || { usage; exit 2; }
[[ "$backup_dir" = /* ]] || { echo 'Use an absolute backup path' >&2; exit 1; }
[[ "$project_name" =~ ^[a-z0-9][a-z0-9_-]*$ ]] || { echo 'Invalid Compose project name' >&2; exit 1; }
[[ -f "$compose_file" && -f .env && -d data/config ]] || { echo 'Missing Compose file, .env, or data/config' >&2; exit 1; }
for artifact in postgres.tar valkey.tar env config Cargo.lock package-lock.json; do
 [[ ! -e "$backup_dir/$artifact" ]] || { echo "Backup destination already contains $artifact; refusing overwrite" >&2; exit 1; }
done
mkdir -m 700 -p "$backup_dir"
compose=(podman compose -f "$compose_file" -p "$project_name")
restart_stack() {
 local status=$?
 if ! "${compose[@]}" start; then
  echo 'Failed to restart Compose services after backup' >&2
  if [[ "$status" == 0 ]]; then status=1; fi
 fi
 exit "$status"
}
if [[ "$leave_stopped" == false ]]; then trap restart_stack EXIT; fi
"${compose[@]}" stop
for volume in postgres valkey; do
 podman volume export "${project_name}_${volume}" --output "$backup_dir/${volume}.tar"
done
cp .env "$backup_dir/env"
cp -r data/config "$backup_dir/config"
cp Cargo.lock web/package-lock.json "$backup_dir/"
chmod -R go-rwx "$backup_dir"
printf 'Backup complete: %s\n' "$backup_dir"
if [[ "$leave_stopped" == true ]]; then printf 'Compose services remain stopped.\n'; fi

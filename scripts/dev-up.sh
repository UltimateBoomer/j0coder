#!/usr/bin/env bash
# Start the complete stack with rootless Podman and no systemd unit.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
[[ $(id -u) != 0 ]] || { echo 'Run development as your normal user' >&2; exit 1; }
podman compose version >/dev/null
command -v curl >/dev/null || { echo 'Missing required command: curl' >&2; exit 1; }
[[ -f .env ]] || { echo 'Run make configure first' >&2; exit 1; }
[[ -x .dev/gvisor/current/runsc ]] || { echo 'Run make dev-gvisor first' >&2; exit 1; }
scripts/gvisor-controller.sh start
cleanup_on_error() {
 trap - ERR INT TERM
 podman compose down >/dev/null 2>&1 || true
 scripts/gvisor-controller.sh stop
}
trap cleanup_on_error ERR
trap 'cleanup_on_error; exit 130' INT
trap 'cleanup_on_error; exit 143' TERM
runtime_dir=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
export PODMAN_SOCKET="$runtime_dir/practice-podman.sock"
export SANDBOX_RUNTIME="$root/.dev/gvisor/current/runsc"
scripts/preflight.sh
podman compose up -d
ready=false
for _ in {1..60}; do
 if curl --fail --silent --connect-timeout 1 --max-time 2 http://127.0.0.1:${PORT:-8080}/healthz >/dev/null; then
  ready=true
  break
 fi
 sleep 0.5
done
if [[ "$ready" != true ]]; then
 echo 'Development stack failed its health check' >&2
 podman compose ps >&2
 false
fi
trap - ERR INT TERM
printf 'Development stack started. Stop it with make dev-down\n'

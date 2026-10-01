#!/usr/bin/env bash
# Start host-native services and loopback-only Compose dependencies.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
[[ $(id -u) != 0 ]] || { echo 'Run development as your normal user' >&2; exit 1; }
for required in podman tmux cargo npm curl; do
 command -v "$required" >/dev/null || { echo "Missing required command: $required" >&2; exit 1; }
done
[[ -f .env ]] || { echo 'Run make configure first' >&2; exit 1; }
[[ -x .dev/gvisor/current/runsc ]] || { echo 'Run make dev-gvisor first' >&2; exit 1; }
compose=(podman compose -f compose.dev.yaml)
tmux_cmd=(tmux -L j0coder-dev)
"${compose[@]}" version >/dev/null
if "${tmux_cmd[@]}" has-session -t j0coder 2>/dev/null; then
 echo 'Development services are already running; use make dev-down before restarting' >&2
 exit 1
fi
requested_toolchain_image=${TOOLCHAIN_IMAGE:-localhost/j0coder-toolchain:1}
set -a
source .env
set +a
export PODMAN_SOCKET="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/j0coder-podman.sock"
export SANDBOX_RUNTIME="$root/.dev/gvisor/current/runsc"
export TOOLCHAIN_IMAGE="$requested_toolchain_image"
if [[ ! -x web/node_modules/.bin/vite || web/package-lock.json -nt web/node_modules/.package-lock.json || web/package.json -nt web/node_modules/.package-lock.json ]]; then
 npm ci --prefix web
fi
cargo build --locked --bin api --bin worker --bin editor --bin catalog-controller
image_id=$(podman image inspect "$TOOLCHAIN_IMAGE" --format '{{.Id}}')
image_id=${image_id#sha256:}
[[ "$image_id" =~ ^[0-9a-f]{64}$ ]] || { echo 'Unexpected toolchain image ID' >&2; exit 1; }
mkdir -p .dev/run/logs
umask 077
printf 'sha256:%s\n' "$image_id" >.dev/run/toolchain-image
for service in api worker editor catalog-controller web; do
 : >".dev/run/logs/$service.log"
done
cleanup_on_error() {
 trap - ERR INT TERM
 scripts/dev-down.sh >/dev/null 2>&1 || true
}
trap cleanup_on_error ERR
trap 'cleanup_on_error; exit 130' INT
trap 'cleanup_on_error; exit 143' TERM
scripts/gvisor-controller.sh start
scripts/preflight.sh
"${compose[@]}" up -d --remove-orphans
database_ready=false
for _ in {1..120}; do
 if "${compose[@]}" exec -T postgres pg_isready -U practice -d practice >/dev/null 2>&1; then
  database_ready=true
  break
 fi
 sleep 0.5
done
[[ "$database_ready" == true ]] || { echo 'PostgreSQL did not become ready' >&2; false; }
for service in api worker editor catalog-controller web; do
 printf -v pane_command '%q %q' "$root/scripts/dev-service.sh" "$service"
 if [[ "$service" == api ]]; then
  "${tmux_cmd[@]}" new-session -d -s j0coder -n "$service" "$pane_command"
  "${tmux_cmd[@]}" set-window-option -g automatic-rename off >/dev/null
  "${tmux_cmd[@]}" set-window-option -g remain-on-exit on >/dev/null
 else
  "${tmux_cmd[@]}" new-window -d -t j0coder -n "$service" "$pane_command"
 fi
done
ready=false
for _ in {1..300}; do
 for service in api worker editor catalog-controller web; do
  pane_dead=$("${tmux_cmd[@]}" list-panes -t "j0coder:$service" -F '#{pane_dead}') || pane_dead=1
  if [[ "$pane_dead" == 1 ]]; then
   echo "$service exited during startup; recent output:" >&2
   tail -n 30 ".dev/run/logs/$service.log" >&2 || true
   false
  fi
 done
 if curl --fail --silent --connect-timeout 1 --max-time 2 "http://127.0.0.1:18080/readyz" >/dev/null \
    && curl --fail --silent --connect-timeout 1 --max-time 2 "http://127.0.0.1:8081/healthz" >/dev/null \
    && curl --fail --silent --connect-timeout 1 --max-time 2 "http://127.0.0.1:${PORT:-8080}/@vite/client" >/dev/null; then
  ready=true
  break
 fi
 sleep 0.5
done
if [[ "$ready" != true ]]; then
 echo 'Native development services failed their health checks' >&2
 for service in api worker editor catalog-controller web; do
  echo "--- $service ---" >&2
  tail -n 15 ".dev/run/logs/$service.log" >&2 || true
 done
 false
fi
trap - ERR INT TERM
printf 'Development services ready at %s\n' "${PUBLIC_ORIGIN:-http://localhost:8080}"
printf 'View logs: tmux -L j0coder-dev attach -t j0coder\n'
printf 'Stop services: make dev-down\n'

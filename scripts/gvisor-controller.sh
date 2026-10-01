#!/usr/bin/env bash
# Manage a project-local rootless Podman API. No systemd unit is required.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
runtime="$root/.dev/gvisor/current/runsc"
runtime_dir=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
socket="$runtime_dir/j0coder-podman.sock"
run_dir="$root/.dev/run"
pid_file="$run_dir/gvisor-controller.pid"
log_file="$run_dir/gvisor-controller.log"

running() {
 [[ -s "$pid_file" ]] || return 1
 local pid
 pid=$(<"$pid_file")
 [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null
}

case "${1:-}" in
 start)
  [[ -x "$runtime" ]] || { echo 'Run scripts/install-gvisor-dev.sh first' >&2; exit 1; }
  if running; then
   printf 'Rootless gVisor controller already running at unix://%s\n' "$socket"
   exit 0
  fi
  mkdir -p "$run_dir"
  rm -f "$pid_file" "$socket"
  nohup podman --runtime="$runtime" system service --time=0 "unix://$socket" \
   >"$log_file" 2>&1 </dev/null &
  pid=$!
  printf '%s\n' "$pid" >"$pid_file"
  for _ in {1..50}; do
   if podman --remote --url "unix://$socket" info >/dev/null 2>&1; then
    printf 'Started rootless gVisor controller at unix://%s\n' "$socket"
    exit 0
   fi
   if ! kill -0 "$pid" 2>/dev/null; then break; fi
   sleep 0.1
  done
  cat "$log_file" >&2
  echo 'Rootless gVisor controller failed to start' >&2
  exit 1
  ;;
 stop)
  if running; then
   pid=$(<"$pid_file")
   kill "$pid"
   for _ in {1..50}; do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
  fi
  rm -f "$pid_file" "$socket"
  echo 'Stopped rootless gVisor controller'
  ;;
 status)
  if running && podman --remote --url "unix://$socket" info >/dev/null 2>&1; then
   printf 'running unix://%s\n' "$socket"
  else
   echo 'stopped'
   exit 1
  fi
  ;;
 *) echo 'Usage: scripts/gvisor-controller.sh {start|stop|status}' >&2; exit 2 ;;
esac

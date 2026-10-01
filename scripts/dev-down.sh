#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
tmux_cmd=(tmux -L j0coder-dev)
if "${tmux_cmd[@]}" has-session -t j0coder 2>/dev/null; then
 for service in api worker editor catalog-controller web; do
  "${tmux_cmd[@]}" send-keys -t "j0coder:$service" C-c >/dev/null 2>&1 || true
 done
 for _ in {1..20}; do
  live=false
  for service in api worker editor catalog-controller web; do
   pane_dead=$("${tmux_cmd[@]}" list-panes -t "j0coder:$service" -F '#{pane_dead}' 2>/dev/null) || continue
   if [[ "$pane_dead" == 0 ]]; then live=true; break; fi
  done
  [[ "$live" == false ]] && break
  sleep 0.5
 done
 "${tmux_cmd[@]}" kill-session -t j0coder >/dev/null 2>&1 || true
fi
status=0
podman compose -f compose.dev.yaml down --remove-orphans || status=$?
scripts/gvisor-controller.sh stop
exit "$status"

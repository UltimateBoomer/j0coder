#!/usr/bin/env bash
# Print shell assignments for the self-contained rootless development runtime.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
runtime_dir=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
printf 'PODMAN_SOCKET=%q\n' "$runtime_dir/locoder-podman.sock"
printf 'SANDBOX_RUNTIME=%q\n' "$root/.dev/gvisor/current/runsc"

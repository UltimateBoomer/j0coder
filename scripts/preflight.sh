#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
requested_socket=${PODMAN_SOCKET:-}
requested_runtime=${SANDBOX_RUNTIME:-}
set -a
source .env
set +a
[[ -z "$requested_socket" ]] || PODMAN_SOCKET=$requested_socket
[[ -z "$requested_runtime" ]] || SANDBOX_RUNTIME=$requested_runtime
runtime=${SANDBOX_RUNTIME:-$PWD/.dev/gvisor/current/runsc}
PODMAN_SOCKET=${PODMAN_SOCKET:-${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/practice-podman.sock}
image=${TOOLCHAIN_IMAGE:-localhost/practice-toolchain:1}
[[ "${runtime##*/}" == runsc ]] || { echo 'Only gVisor runsc is supported' >&2; exit 1; }
podman run --rm --runtime "$runtime" --network=none --read-only --read-only-tmpfs=false \
 --user=65534:65534 --cap-drop=ALL --security-opt=no-new-privileges --security-opt=label=disable \
 --memory=256m --memory-swap=256m --cpus=1 --pids-limit=64 --timeout=10 \
 "$image" dmesg | grep -q gVisor
printf 'Podman + gVisor resource-constrained sandbox passed\n'
limit_name="practice-limit-check-$$"
cleanup_limit() { podman rm -f "$limit_name" >/dev/null 2>&1 || true; }
trap cleanup_limit EXIT
podman run -d --name "$limit_name" --runtime "$runtime" --network=none \
 --security-opt=label=disable --memory=256m --memory-swap=256m --cpus=1 --pids-limit=64 \
 "$image" sleep 30 >/dev/null
limit_pid=$(podman inspect "$limit_name" --format '{{.State.Pid}}')
cgroup_path=$(awk -F: '$1 == "0" {print $3}' "/proc/$limit_pid/cgroup")
cgroup_dir="/sys/fs/cgroup$cgroup_path"
memory_max=$(<"$cgroup_dir/memory.max")
pids_max=$(<"$cgroup_dir/pids.max")
read -r cpu_quota cpu_period <"$cgroup_dir/cpu.max"
[[ "$memory_max" == 268435456 ]] || { echo "gVisor memory limit missing: $memory_max" >&2; exit 1; }
[[ "$pids_max" == 64 ]] || { echo "gVisor PID limit missing: $pids_max" >&2; exit 1; }
[[ "$cpu_quota" != max && "$cpu_quota" -le "$cpu_period" ]] || { echo "gVisor CPU limit missing: $cpu_quota $cpu_period" >&2; exit 1; }
cleanup_limit
trap - EXIT
printf 'gVisor cgroup limits are enforced\n'
server_runtime=$(podman --remote --url "unix://${PODMAN_SOCKET}" info --format '{{.Host.OCIRuntime.Name}}')
[[ "${server_runtime##*/}" == runsc ]] || { echo 'Dedicated Podman server is not configured for runsc' >&2; exit 1; }
printf 'Dedicated controller socket selects runsc\n'

#!/usr/bin/env bash
# Run one trusted service on the host with the development dependencies.
set -euo pipefail
if [[ $# -lt 1 ]]; then
 echo 'Usage: scripts/dev-service.sh {api|worker|editor|catalog-controller|web} [args]' >&2
 exit 2
fi
service=$1
shift
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if [[ $# == 0 ]]; then
 mkdir -p "$root/.dev/run/logs"
 exec > >(tee -a "$root/.dev/run/logs/$service.log") 2>&1
fi
set -a
source .env
set +a
export DATABASE_URL="postgres://practice:${POSTGRES_PASSWORD}@127.0.0.1:15432/practice"
export VALKEY_URL="redis://:${VALKEY_PASSWORD}@127.0.0.1:16379"
export CONTAINER_HOST="unix://${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/locoder-podman.sock"
export SANDBOX_RUNTIME="$root/.dev/gvisor/current/runsc"
toolchain_image=$(<"$root/.dev/run/toolchain-image")
export TOOLCHAIN_IMAGE="$toolchain_image"
export CATALOG_SSH_KEY_PATH=${CATALOG_SSH_KEY_PATH:-/dev/null}
export CATALOG_KNOWN_HOSTS_PATH=${CATALOG_KNOWN_HOSTS_PATH:-/dev/null}
case "$service" in
 api) export API_BIND=127.0.0.1:18080; exec "$root/target/debug/api" "$@" ;;
 worker) exec "$root/target/debug/worker" "$@" ;;
 editor) export EDITOR_BIND=127.0.0.1:8081; exec "$root/target/debug/editor" "$@" ;;
 catalog-controller) exec "$root/target/debug/catalog-controller" "$@" ;;
 web) exec npm run dev --prefix web -- "$@" ;;
 *) echo 'Expected api, worker, editor, catalog-controller, or web' >&2; exit 2 ;;
esac

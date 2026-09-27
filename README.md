# Locoder

Locoder is a self-hosted coding workspace for authoring and solving programming problems. It combines a Svelte/Monaco frontend, a Rust/Axum API, PostgreSQL, Valkey Streams, and isolated gVisor execution through either rootless Podman or Kubernetes.

Problems can expose global C++ or top-level Python functions, or stateful data structures. Published versions are immutable, hidden tests remain server-side, and every test runs inside a resource-limited sandbox.

> gVisor is mandatory for untrusted execution. Local development uses rootless Podman for PostgreSQL, Valkey, and sandbox execution, with the project-local patched runsc runtime. Staging and production use Kubernetes with an operator-installed gVisor RuntimeClass.

## Develop locally

This is the default development path. It runs as your current Linux user and does not install a system service. Install Podman, a `podman compose` provider (such as podman-compose), tmux, Rust/Cargo, Node.js/npm, Git, curl, and the host user-namespace helpers.

```sh
make dev-up
```

On the first run, Locoder creates `.env`, builds the sandbox toolchain image and a pinned gVisor release, installs frontend dependencies, and builds the Rust binaries. Network access is required and setup may take a while. Later starts reuse the local build caches.

PostgreSQL and Valkey run in `compose.dev.yaml`, with ports published only on `127.0.0.1`. The API, worker, editor, catalog controller, and Vite run natively in a tmux session. Open `http://localhost:8080`; changes under `web/` hot reload there. Vite proxies API requests and editor WebSockets to the native services. To inspect each service's output:

```sh
make dev-attach
```

Switch windows with `Ctrl-b n`, or read the logs under `.dev/run/logs/`. After changing a Rust service, rebuild and restart its window. For example:

```sh
cargo build --locked --bin api
tmux -L locoder-dev respawn-window -k -t locoder:api
```

To run all Locoder services with the full `compose.yaml` stack on one rootless host, use:

```sh
make images
make dev-gvisor
scripts/gvisor-controller.sh start
make up
```

Create the first administrator after the services are ready:

```bash
read -rsp 'New admin password (12+ characters): ' locoder_password; echo
printf '%s\n' "$locoder_password" | scripts/dev-service.sh api bootstrap-admin admin
unset locoder_password
```

In fish:

```fish
read --silent --prompt-str 'New admin password (12+ characters): ' locoder_password
echo
printf '%s\n' "$locoder_password" | scripts/dev-service.sh api bootstrap-admin admin
set -e locoder_password
```

Sign in as `admin`. Stop the native services, Compose dependencies, and dedicated Podman controller with:

```sh
make dev-down
```

Run `make help` for all setup, build, verification, and lifecycle targets.

### Local configuration

`scripts/configure.py` creates `.env` with random PostgreSQL and Valkey credentials, a loopback public origin, and rootless runtime paths. It refuses to overwrite an existing configuration.

After signing in, open **Authoring** and publish a problem. Sample definitions can be imported with:

```sh
PUBLIC_ORIGIN=http://localhost:8080 python3 scripts/seed.py
```

The Git-managed catalog is optional. Its current controller accepts SSH repository URLs only; see [catalog documentation](docs/catalog.md).

### Local runtime boundary

Native services and the Compose dependencies listen on loopback only. The worker and editor reach a separate rootless Podman API socket whose default runtime is patched runsc. Startup preflight verifies gVisor and its memory, CPU, and PID enforcement. Never expose PostgreSQL, Valkey, or the Podman socket.

## Staging and production on Kubernetes

Use the [Helm deployment guide](deploy/helm/locoder/README.md) for prerequisites, image digests, external PostgreSQL and Valkey, Secrets, ingress, first administrator setup, upgrades, and rollback. Set `publicOrigin` to the browser's HTTPS origin and use a TLS ingress. The chart requires a working gVisor RuntimeClass and never falls back to the node's ordinary runtime.

To exercise the Kubernetes deployment locally on Fedora, see the [disposable Lima cluster guide](docs/local-kubernetes-fedora.md). Its database uses ephemeral storage and is not a staging or production deployment.

See [architecture and scaling](docs/architecture.md) for service boundaries, execution flows, failure recovery, and deployment topology.

## Problem contracts

Schema 3 supports two interfaces:

- `function`: a global C++ function or top-level Python function.
- `data_structure`: a named class with a constructor and ordered method traces.

Every problem must declare `schema: 3` and an `interface`; legacy `signature` fields are rejected. Types include integers, floats, booleans, strings, arrays, nullable values, and declared structured types. `void` is limited to data-structure method returns.

Each function case receives a fresh sandbox. Each stateful case constructs one fresh instance and executes its complete operation trace under one time and memory limit. Hidden cases expose only their designation and verdict.

The versioned REST contract is [openapi.json](openapi.json), also served at `/api/v1/openapi.json`.

## Development and verification

Run the standard local checks with:

```sh
make check
```

The equivalent commands are:

```sh
cargo build --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
npm ci --prefix web
npm run check --prefix web
npm run build --prefix web
npm run generate:api --prefix web
```

Build and pin both local container images with:

```sh
make images BUILD_JOBS=2
```

Publishing a GitHub Release runs `.github/workflows/publish-images.yml`. It builds `deploy/App.Containerfile` and `deploy/Toolchain.Containerfile` for `linux/amd64`, then publishes `ghcr.io/ultimateboomer/locoder-app:<release-tag>` and `ghcr.io/ultimateboomer/locoder-toolchain:<release-tag>`. The workflow summary records both immutable digests for the Helm values file. It uses the repository's `GITHUB_TOKEN` with `packages: write`; no personal access token is needed for publishing.

When updating the checked-in REST schema, run `python3 scripts/generate-openapi.py` before `npm run generate:api --prefix web`.

Integration and browser tests require isolated PostgreSQL/Valkey services or a complete deployment. See [testing](docs/testing.md) and the current [verification record](docs/verification.md).

## Backup and restore

For a full Compose installation, back up PostgreSQL and Valkey together using the same rootless user and Compose project:

```sh
scripts/backup.sh "$HOME/backups/locoder-$(date +%F)"
```

Restore only into a fresh checkout with no `.env` and no `locoder_*` volumes:

```sh
scripts/restore.sh "$HOME/backups/locoder-2026-09-20"
make images
make dev-gvisor
scripts/gvisor-controller.sh start
make up
```

Protect backups as secrets. For staging and production, back up the external PostgreSQL and Valkey services according to their operators' procedures and retain the deployed image digests.

## Documentation

- [Architecture and scaling](docs/architecture.md)
- [Git-managed catalog](docs/catalog.md)
- [Problem curation plan](docs/problem-curation-plan.md)
- [Testing](docs/testing.md)
- [Verification record](docs/verification.md)

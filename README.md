# Locoder

Locoder is a self-hosted coding workspace for authoring and solving programming problems. It combines a Svelte/Monaco frontend, a Rust/Axum API, PostgreSQL, Valkey Streams, and isolated gVisor execution through either rootless Podman or Kubernetes.

Problems can expose global C++ or top-level Python functions, or stateful data structures. Published versions are immutable, hidden tests remain server-side, and every test runs inside a resource-limited sandbox.

> gVisor is mandatory for untrusted execution. Local development uses rootless Podman Compose and the project-local patched runsc runtime. Staging and production use Kubernetes with an operator-installed gVisor RuntimeClass.

## Develop locally with Podman Compose

This is the default development path. It runs as your current Linux user and does not install a system service. Install Podman, a `podman compose` provider (such as podman-compose), Git, curl, and the host user-namespace helpers. The images use Docker-compatible container formats, but the local startup scripts and sandbox controller currently require Podman.

```sh
make dev-up
```

On the first run, Locoder creates `.env`, builds the application and toolchain images and a pinned gVisor release, then starts the Compose stack. Network access is required and setup may take a while. Build artifacts and the runtime stay under the ignored `.dev/` directory; later starts reuse the local cache.

Create the first administrator after the stack is ready:

```bash
read -rsp 'New admin password (12+ characters): ' locoder_password; echo
printf '%s\n' "$locoder_password" | podman compose exec -T api api bootstrap-admin admin
unset locoder_password
```

In fish:

```fish
read --silent --prompt-str 'New admin password (12+ characters): ' locoder_password
echo
printf '%s\n' "$locoder_password" | podman compose exec -T api api bootstrap-admin admin
set -e locoder_password
```

Open `http://localhost:8080` and sign in as `admin`. Stop the stack and its dedicated Podman controller with:

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

The Compose stack defaults to loopback-only HTTP. Ordinary services use the current user's default OCI runtime. The worker and editor reach a separate rootless Podman API socket whose default runtime is patched runsc. Startup preflight verifies gVisor and its memory, CPU, and PID enforcement. Never expose PostgreSQL, Valkey, or the Podman socket.

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

CI may import and export layers with `CACHE_FROM_REPO` and `CACHE_TO_REPO`. Keep cache repositories private because intermediate layers can contain source files.

Integration and browser tests require isolated PostgreSQL/Valkey services or a complete deployment. See [testing](docs/testing.md) and the current [verification record](docs/verification.md).

## Backup and restore

For a local Compose installation, back up PostgreSQL and Valkey together using the same rootless user and Compose project:

```sh
scripts/backup.sh "$HOME/backups/locoder-$(date +%F)"
```

Restore only into a fresh checkout with no `.env` and no `locoder_*` volumes:

```sh
scripts/restore.sh "$HOME/backups/locoder-2026-09-20"
make dev-up
```

Protect backups as secrets. For staging and production, back up the external PostgreSQL and Valkey services according to their operators' procedures and retain the deployed image digests.

## Documentation

- [Architecture and scaling](docs/architecture.md)
- [Git-managed catalog](docs/catalog.md)
- [Testing](docs/testing.md)
- [Verification record](docs/verification.md)

# Locoder

Locoder is a self-hosted coding workspace for authoring and solving programming problems. It combines a Svelte/Monaco frontend, a Rust/Axum API, PostgreSQL, Valkey Streams, and isolated Podman + gVisor execution.

Problems can expose global C++ or top-level Python functions, stateful data structures, or the legacy `Solution` class contract. Published versions are immutable, hidden tests remain server-side, and every test runs inside a resource-limited sandbox.

> Rootless Podman with the patched gVisor runtime is the only supported execution model. Do not substitute crun/runc or disable cgroup enforcement.

## Quick start

The development stack runs as the current Linux user and does not install a system service. You need Podman, podman-compose, Git, curl, and the host user-namespace helpers.

```sh
make dev-up
```

On the first run, Locoder builds its application images and a pinned gVisor release, so network access is required and setup may take a while. Build artifacts and the runtime stay under the ignored `.dev/` directory. Later starts reuse the local cache.

Open `http://localhost:8080`. Stop the stack and its dedicated Podman controller with:

```sh
make dev-down
```

Run `make help` for all setup, build, verification, and lifecycle targets.

## Initial setup

`scripts/configure.py` creates `.env` with random PostgreSQL and Valkey credentials, a loopback public origin, and rootless runtime paths. It refuses to overwrite an existing configuration.

Create the first administrator without placing the password in process arguments:

```sh
read -rsp 'New admin password (12+ characters): ' locoder_password; echo
printf '%s\n' "$locoder_password" | podman compose exec -T api api bootstrap-admin admin
unset locoder_password
```

Sign in, open **Authoring**, and publish a problem. Sample definitions can be imported with:

```sh
PUBLIC_ORIGIN=http://localhost:8080 python3 scripts/seed.py
```

The Git-managed catalog is optional. Its current controller accepts SSH repository URLs only; see [catalog documentation](docs/catalog.md).

## Deployment

Locoder defaults to loopback-only HTTP. For remote use, place an HTTPS reverse proxy in front of it, set `PUBLIC_ORIGIN` to the exact browser origin, enable `COOKIE_SECURE`, and support WebSockets. Never expose PostgreSQL, Valkey, or the Podman socket.

Ordinary services use the current user's default OCI runtime. The worker and editor reach a separate rootless Podman API socket whose default runtime is patched runsc. Startup preflight verifies gVisor and its memory, CPU, and PID enforcement.

Use a dedicated operating-system account in production. Access to the Podman controller grants authority over that user's container engine; only trusted Locoder services should receive the socket.

See [architecture and scaling](docs/architecture.md) for service boundaries, execution flows, failure recovery, and deployment topology.

## Problem contracts

Schema 3 supports two interfaces:

- `function`: a global C++ function or top-level Python function.
- `data_structure`: a named class with a constructor and ordered method traces.

Schema 1 and 2 `Solution` class problems remain supported. Types include integers, floats, booleans, strings, arrays, nullable values, and declared structured types. `void` is limited to data-structure method returns.

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

Back up PostgreSQL and Valkey together using the same rootless user and Compose project:

```sh
scripts/backup.sh "$HOME/backups/locoder-$(date +%F)"
```

Restore only into a fresh checkout with no `.env` and no `locoder_*` volumes:

```sh
scripts/restore.sh "$HOME/backups/locoder-2026-09-20"
make dev-up
```

Protect backups as secrets. Archive the built container images alongside production backups when reproducible recovery is required.

## Documentation

- [Architecture and scaling](docs/architecture.md)
- [Git-managed catalog](docs/catalog.md)
- [Testing](docs/testing.md)
- [Verification record](docs/verification.md)

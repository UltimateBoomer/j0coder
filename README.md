# j0coder

j0coder is a private, self-hostable alternative to LeetCode and HackerRank. It gives you a place to practice programming, prepare for interviews, and publish your own coding problems while keeping accounts, solutions, and test data on infrastructure you control.

Use it as a personal practice workspace or host a shared problem library for a team or class. Learners solve problems in the browser; administrators curate and publish the problems available on their installation.

## What you can do

- **Practice in four languages:** solve problems in C++, Python, Java, and Kotlin with generated starter code.
- **Code in your browser:** use an editor with syntax highlighting, language-aware completion, and diagnostics.
- **Pick your next challenge:** search the problem library and filter by difficulty and tags. Blind mode hides difficulty and tags while you solve.
- **Learn at your own pace:** read examples and constraints, and reveal hints one at a time.
- **Save your progress:** keep solution drafts per language and revisit your run history.
- **Get feedback:** run solutions against visible and hidden tests, inspect verdicts and diagnostics, and compare visible outputs with expected results.
- **Create your own library:** author and publish problems through the web interface, or maintain an optional Git-managed catalog. Problems can test functions or stateful data structures.
- **Keep tests private:** hidden inputs and expected outputs stay on the server. Published problem versions are immutable, and code runs in isolated sandboxes with time and memory limits.

## Set up your own installation

Choose a single-host Compose installation or Kubernetes. Both require gVisor for running submitted code and editor language services.

### Production with Docker/Podman Compose

The supplied `compose.yaml` runs the full application, PostgreSQL, Valkey, and a web proxy on one x86_64 Linux host. **The supported container engine is rootless Podman.** Although this is a Compose deployment, the file uses Podman-specific user mappings and a Podman API socket; it is not a drop-in Docker Engine deployment. Docker Engine support would require adapting those settings and providing a compatible sandbox backend.

Install Git, Make, Python 3, Podman, a `podman compose` provider (such as podman-compose), and the host user-namespace helpers. The [gVisor installer](scripts/install-gvisor-dev.sh) uses a containerized builder and requires `crun`, `tar`, and `sha256sum`. Run the following from a checkout as the Linux user that will own the installation:

```sh
make configure
make images BUILD_JOBS=2
make dev-gvisor
scripts/gvisor-controller.sh start
```

Configuration generates random database and queue credentials in `.env` and `data/config/`. Image builds pin the application and toolchain image IDs in `.env`. Initial setup requires network access and can take a while.

Before starting, edit `.env` for your deployment:

```dotenv
PUBLIC_ORIGIN=https://coding.example.com
COOKIE_SECURE=true
PORT=8080
CATALOG_ENABLED=false
CATALOG_SSH_KEY_PATH=./data/config/catalog-disabled-key
CATALOG_KNOWN_HOSTS_PATH=./data/config/catalog-disabled-known-hosts
```

The catalog mount paths are required even when Git import is disabled. Create empty placeholder files for this configuration:

```sh
touch data/config/catalog-disabled-key data/config/catalog-disabled-known-hosts
```

To enable Git import, supply a deploy key and known-hosts file instead; follow the [catalog guide](docs/catalog.md).

Place a TLS reverse proxy in front of `127.0.0.1:8080`, forwarding browser traffic and editor WebSockets. Set `PUBLIC_ORIGIN` to the exact browser-facing HTTPS origin. PostgreSQL and Valkey stay on internal container networks; keep the Podman socket private.

```sh
make up
podman compose ps
podman compose logs --tail=100 api worker editor
```

`make up` checks gVisor and resource-limit enforcement before starting the stack. Once the API is ready, create the first administrator (Bash):

```bash
read -rsp 'New admin password (12+ characters): ' j0coder_password; echo
printf '%s\n' "$j0coder_password" | podman compose exec -T api api bootstrap-admin admin
unset j0coder_password
```

Sign in as `admin` at your configured public origin and open **Authoring** to publish your first problem. The bootstrap command is for a new database; an existing username causes it to fail.

To stop the installation without deleting its database and queue volumes:

```sh
podman compose down
scripts/gvisor-controller.sh stop
```

Arrange for the dedicated Podman controller and Compose stack to start again after host reboots. The setup commands do not install a host startup service.

### Production with Kubernetes

The [Helm deployment guide](deploy/helm/j0coder/README.md) covers the complete installation, credentials, ingress, capacity, upgrades, and rollback. The chart installs the application services and a dedicated sandbox namespace; you provide:

- A Kubernetes cluster with NetworkPolicy enforcement and an installed gVisor RuntimeClass.
- External PostgreSQL and Valkey services, with connection URLs in existing Kubernetes Secrets.
- Application and toolchain images pinned to immutable digests, plus pull credentials if the images are private.
- A TLS ingress and the HTTPS origin users will visit.

Copy the chart's [values file](deploy/helm/j0coder/values.yaml) to `production-values.yaml`. Configure `publicOrigin`, ingress host and TLS Secret, image repositories and digests, the gVisor RuntimeClass, database and queue Secret references, and network destinations for your cluster. Complete the prerequisites in the Helm guide, then install:

```sh
helm upgrade --install j0coder deploy/helm/j0coder \
  --namespace j0coder --create-namespace -f production-values.yaml
kubectl -n j0coder rollout status deployment/j0coder-api
```

Create the first administrator (Bash):

```bash
read -rsp 'New admin password (12+ characters): ' j0coder_password; echo
printf '%s\n' "$j0coder_password" | kubectl -n j0coder exec -i deployment/j0coder-api -- api bootstrap-admin admin
unset j0coder_password
```

Sign in at your configured public origin. Adjust the namespace and Deployment name if you choose another release name. Workers and editors require a working gVisor RuntimeClass and never fall back to the node's ordinary runtime.

For a disposable test cluster, see [Local Kubernetes on Fedora](docs/local-kubernetes-fedora.md). Its database uses ephemeral storage; use the Helm production guide for a persistent installation.

### Backup and restore

For a full Compose installation, back up PostgreSQL and Valkey together using the same rootless user and Compose project:

```sh
scripts/backup.sh "$HOME/backups/j0coder-$(date +%F)"
```

Restore only into a fresh checkout with no `.env` and no `j0coder_*` volumes:

```sh
scripts/restore.sh "$HOME/backups/j0coder-2026-09-20"
make images
make dev-gvisor
scripts/gvisor-controller.sh start
make up
```

Protect backups as secrets. For Kubernetes installations, back up the external PostgreSQL and Valkey services according to their operators' procedures and retain the deployed image digests.

## Development

The application uses a Svelte/Monaco frontend, a Rust/Axum API, PostgreSQL, Valkey Streams, and gVisor sandboxes. See [architecture and scaling](docs/architecture.md) for service boundaries and execution flows.

### Develop locally

This is the default development path. It runs as your current Linux user and does not install a system service. Install Podman, a `podman compose` provider (such as podman-compose), tmux, Rust/Cargo, Node.js/npm, Git, curl, and the host user-namespace helpers.

```sh
make dev-up
```

On the first run, j0coder creates `.env`, builds the sandbox toolchain image and a pinned gVisor release, installs frontend dependencies, and builds the Rust binaries. Network access is required and setup may take a while. Later starts reuse the local build caches.

PostgreSQL and Valkey run in `compose.dev.yaml`, with ports published only on `127.0.0.1`. The API, worker, editor, catalog controller, and Vite run natively in a tmux session. Open `http://localhost:8080`; changes under `web/` hot reload there. Vite proxies API requests and editor WebSockets to the native services. To inspect each service's output:

```sh
make dev-attach
```

Switch windows with `Ctrl-b n`, or read the logs under `.dev/run/logs/`. After changing a Rust service, rebuild and restart its window. For example:

```sh
cargo build --locked --bin api
tmux -L j0coder-dev respawn-window -k -t j0coder:api
```

Create the first administrator after the services are ready:

```bash
read -rsp 'New admin password (12+ characters): ' j0coder_password; echo
printf '%s\n' "$j0coder_password" | scripts/dev-service.sh api bootstrap-admin admin
unset j0coder_password
```

In fish:

```fish
read --silent --prompt-str 'New admin password (12+ characters): ' j0coder_password
echo
printf '%s\n' "$j0coder_password" | scripts/dev-service.sh api bootstrap-admin admin
set -e j0coder_password
```

Sign in as `admin`. Stop the native services, Compose dependencies, and dedicated Podman controller with:

```sh
make dev-down
```

Run `make help` for all setup, build, verification, and lifecycle targets.

### Development configuration

`scripts/configure.py` creates `.env` with random PostgreSQL and Valkey credentials, a loopback public origin, and rootless runtime paths. It refuses to overwrite an existing configuration.

After signing in, open **Authoring** and publish a problem. Sample definitions can be imported with:

```sh
PUBLIC_ORIGIN=http://localhost:8080 python3 scripts/seed.py
```

The Git-managed catalog is optional. Its current controller accepts SSH repository URLs only; see [catalog documentation](docs/catalog.md).

### Development runtime boundary

Native services and the Compose dependencies listen on loopback only. The worker and editor reach a separate rootless Podman API socket whose default runtime is patched runsc. Startup preflight verifies gVisor and its memory, CPU, and PID enforcement. Never expose PostgreSQL, Valkey, or the Podman socket.

### Build and verify

Run the standard local checks with:

```sh
make check
```

For a broader build and verification pass, run:

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

Publishing a GitHub Release runs `.github/workflows/publish-images.yml`. It builds `deploy/App.Containerfile` and `deploy/Toolchain.Containerfile` for `linux/amd64`, then publishes `ghcr.io/ultimateboomer/j0coder-app:<release-tag>` and `ghcr.io/ultimateboomer/j0coder-toolchain:<release-tag>`. The workflow summary records both immutable digests for the Helm values file. It uses the repository's `GITHUB_TOKEN` with `packages: write`; no personal access token is needed for publishing.

When updating the checked-in REST schema, run `python3 scripts/generate-openapi.py` before `npm run generate:api --prefix web`.

Integration and browser tests require isolated PostgreSQL/Valkey services or a complete deployment. See [testing](docs/testing.md) and the current [verification record](docs/verification.md).

### Problem contracts

Schema 3 supports two interfaces:

- `function`: a function with language-specific starters for C++, Python, Java, and Kotlin.
- `data_structure`: a named class with a constructor and ordered method traces.

Every problem must declare `schema: 3` and an `interface`; legacy `signature` fields are rejected. Types include integers, floats, booleans, strings, arrays, nullable values, and declared structured types. `void` is limited to data-structure method returns.

Each function case receives a fresh sandbox. Each stateful case constructs one fresh instance and executes its complete operation trace under one time and memory limit. Hidden cases expose only their designation and verdict.

The versioned REST contract is [openapi.json](openapi.json), also served at `/api/v1/openapi.json`.

## Documentation

- [Architecture and scaling](docs/architecture.md)
- [Git-managed catalog](docs/catalog.md)
- [Problem curation plan](docs/problem-curation-plan.md)
- [Testing](docs/testing.md)
- [Verification record](docs/verification.md)

# j0coder

j0coder is a self-hostable alternative to LeetCode and HackerRank. It gives you a place to practice programming, prepare for interviews, and publish your own coding problems while keeping accounts, solutions, and test data on infrastructure you control.

Use it as a personal practice workspace or host a shared problem library for a team or class. Learners solve problems in the browser; administrators curate and publish the problems available on their installation.

## What you can do

- **Practice in four languages:** solve problems in C++, Python, Java, and Kotlin with generated starter code.
- **Code in your browser:** use an editor with syntax highlighting, language-aware completion, and diagnostics.
- **Pick your next challenge:** search the problem library and filter by difficulty and tags. Blind mode hides difficulty and tags while you solve.
- **Learn at your own pace:** read examples and constraints, and reveal hints one at a time.
- **Save your progress:** keep solution drafts per language and revisit your run history.
- **Get feedback:** run solutions against visible and hidden tests, inspect verdicts and diagnostics, and compare visible outputs with expected results.
- **Create your own library:** author and publish problems through the host CLI or optional private web interface, or maintain an optional Git-managed catalog. Problems can test functions or stateful data structures.
- **Keep tests private:** hidden inputs and expected outputs stay on the server. Published problem versions are immutable, and code runs in isolated sandboxes with time and memory limits.

## Set up your own installation

Choose a single-host Compose installation or Kubernetes. Both require gVisor for running submitted code and editor language services.

### Production with Docker/Podman Compose

The supplied `compose.yaml` runs the full application, PostgreSQL, Valkey, and a web proxy on one x86_64 Linux host. **The supported container engine is rootless Podman.** Although this is a Compose deployment, the file uses Podman-specific user mappings and a Podman API socket; it is not a drop-in Docker Engine deployment. Docker Engine support would require adapting those settings and providing a compatible sandbox backend.

From a checkout, run:

```sh
make setup
```

Or install without Git or a build toolchain:

```sh
curl -fsSL https://raw.githubusercontent.com/UltimateBoomer/j0coder/main/scripts/install.sh | sh
```

To inspect the entry point first:

```sh
curl -fsSL https://raw.githubusercontent.com/UltimateBoomer/j0coder/main/scripts/install.sh -o install.sh
less install.sh
sh install.sh
```

The installer requires Python 3.9+, Linux x86_64, rootless Podman with a Compose provider, user namespaces, systemd user services, and cgroup v2. It offers apt/dnf prerequisite installation on Debian/Ubuntu and Fedora, showing commands before running sudo. Package installation and reboot integration are opt-in. Other distributions must install prerequisites themselves.

First setup resolves the latest complete stable release, downloads checksum-verified patched gVisor and deployment archives, pulls immutable application/toolchain images, generates database and queue credentials, runs resource-limit preflight, and waits for readiness. A release becomes installable only after its compatible `release-manifest.json` is published. Older image-only releases cannot be installed this way.

Terminal prompts use `/dev/tty`, including hidden and confirmed administrator password entry, so piped installation works. The default origin is `http://localhost:8080`, with ingress bound to loopback. For a public installation, choose the exact HTTPS browser origin and put your existing TLS reverse proxy in front of the local port; forward editor WebSockets too. Cookie security follows the chosen origin. Setup asks for the problem Git repository, defaulting to `j0team/j0coder-problems`; GitHub `owner/repository` entries are stored as SSH URLs. Use `--catalog-repository` or `CATALOG_REPOSITORY_URL` to supply it without a prompt. Git import is disabled by default and placeholder mounts are created automatically. Follow the [catalog guide](docs/catalog.md) to enable import.

The default installation directory is `$HOME/.local/share/j0coder`. Installer arguments include `--install-dir`, `--version`, `--public-origin`, `--port`, `--admin-username`, `--admin-password-file`, `--install-packages`/`--no-install-packages`, `--autostart`/`--no-autostart`. Environment equivalents are `J0CODER_INSTALL_DIR`, `J0CODER_VERSION`, `PUBLIC_ORIGIN`, `PORT`, `ADMIN_USERNAME`, and `ADMIN_PASSWORD_FILE`. Setup always assumes an interactive terminal and reads missing answers through `/dev/tty`, including hidden password entry. Supplied arguments and environment values bypass their corresponding prompts. Package installation and reboot integration require explicit flags or confirmation at their prompts:

```sh
sh install.sh --no-install-packages --no-autostart \
  --public-origin https://coding.example.com --port 8080 \
  --admin-username admin --admin-password-file /path/to/private/password
```

The administrator password is sent through stdin and never written to `.env`, process arguments or logs. Usernames accept 1–64 ASCII letters, digits, underscores and hyphens; passwords accept 12–256 bytes. Sign in at the printed URL and open **Authoring** to publish your first problem.

Run lifecycle commands inside the installation directory:

```sh
make setup
# Optional setup flags, including selecting this beta:
make setup SETUP_ARGS="--version v0.1.1-beta.1"
# Start an existing installation:
make up
make status
make logs
make down
```

Repeated startup retains release pins, credentials, accounts and persistent volumes. Missing derived configuration is repaired using existing secrets. Incomplete credential configuration is refused; restore the original `.env` before retrying. Failures name the stage; rerun `make setup` to resume. `make down` preserves database and queue volumes and stops the user service without disabling it. Restored databases retain their administrators. Existing users with no administrator require deliberate account recovery; setup will not create another account automatically. `api bootstrap-status` and `api bootstrap-admin USERNAME --if-empty` expose the same database-based safeguards locally.

After successful setup you can opt into `j0coder.service`, a systemd user service that supervises the dedicated controller and recreates Compose containers after controller restarts. Reboot startup uses cached pinned artifacts without package installation, upgrades or onboarding. User lingering is needed to start before login; the installer offers the `sudo loginctl enable-linger` command and reports whether reboot setup is complete. Use `systemctl --user disable --now j0coder.service` to disable autostart. One installation per Linux user is supported.

Native development (`make dev-up`) and Kubernetes remain separate workflows. The release publisher builds both images and patched gVisor, verifies anonymous image pulls and archive downloads, and publishes the completion manifest last. GHCR packages must be public for that gate to pass.

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
make setup
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
- [Account security, guest mode, settings, and operations](docs/security.md)
- [Small-scale hosting, identity, and anti-bot research](docs/hosting.md)
- [Hetzner and reusable Terraform deployment](deploy/terraform/README.md)
- [Git-managed catalog](docs/catalog.md)
- [Problem curation plan](docs/problem-curation-plan.md)
- [Testing](docs/testing.md)
- [Verification record](docs/verification.md)

Setup and lifecycle commands:

```sh
make setup SETUP_ARGS="--version v0.1.1-beta.1"  # install or resume onboarding
make up                                       # start configured installation
make modify SETUP_ARGS="--port 8081 --public-origin https://practice.example"
make upgrade SETUP_ARGS="--version <release-tag>"
make uninstall                                # confirm removal of services; retain data
make uninstall SETUP_ARGS="--purge"            # confirm deletion of volumes and configuration
```

`modify` prompts for unspecified origin and port, preserves database and queue passwords, and restarts services. Use `--autostart` or `--no-autostart` to enable or disable reboot startup. Administrator credentials are managed through the host CLI; modify does not reset accounts. `up` uses cached configuration and does not perform installation or onboarding.

Upgrade is supported for downloaded deployments; source checkouts are updated with Git. It verifies archives and pulls images before stopping services, retains a previous-release file backup, and preserves persistent volumes. Back up PostgreSQL before upgrading: database migrations can prevent a downgrade. An omitted version selects the latest complete stable release.

Uninstall disables and removes the user service, stops the controller, and removes Compose containers and networks. Files remain available for reinstalling with `make setup`; `--purge` additionally deletes persistent volumes and generated configuration/runtime files. It does not remove system packages or user lingering. Use `--yes` to explicitly confirm either uninstall operation without its confirmation prompt.

The downloadable installer accepts `--action modify`, `--action upgrade`, or `--action uninstall`, together with `--install-dir` pointing to the existing deployment. Its default action remains installation.

Setup prints eight numbered stages and pauses for missing answers through `/dev/tty`, including when the downloaded installer is piped to `sh`. Enter accepts the displayed default; yes/no prompts also accept y/n. Password entry is hidden. Routine command diagnostics are saved in private per-run logs under `<installation>/.dev/setup-logs/` (directory 0700, files 0600). Machine-readable inspection output and credential input are excluded; known passwords are redacted.

For live diagnostic output:

```sh
make setup SETUP_ARGS="--verbose"
sh install.sh --verbose
```

Failures show the stage, recent diagnostics, log location, and an action-specific recovery command. `make status` and `make logs` continue to display their output, and the reboot supervisor writes diagnostics to the systemd journal.

# Locoder

A self-hosted coding workspace with a Svelte/Monaco browser interface, Rust/Axum API, PostgreSQL, Valkey Streams, and **Podman + gVisor** execution. Locoder supports free functions, stateful data structures, and legacy `Solution` problems in C++20 and Python. There is no frontend Node server.

**Release status:** application and container builds, API/browser checks, real Valkey recovery checks, both language wrapper fixtures, and rootless gVisor judging have been verified. Isolated semantic completion and production deployment are implemented but **not deployment-certified**. See [verification](docs/verification.md) for exact evidence and remaining acceptance gates. Do not substitute crun/runc for runsc.

## Rootless Linux development

The complete development stack can run under the current user without installing a systemd unit. The project builds a tag- and image-digest-pinned gVisor release with the rootless cgroup patches in `deploy/gvisor/`, installs it under the ignored `.dev/` directory, starts a dedicated rootless Podman API process, verifies real memory/CPU/PID enforcement, and then starts Compose:

```sh
make dev-up
```

`dev-up` builds and pins the application images before starting the stack, so source changes cannot accidentally run against stale images. The first run needs network access and takes time because it also clones gVisor and builds it in gVisor's digest-pinned canonical build image; later builds use the local cache. No global compiler, Bazel, or mise installation is required. Source, build cache, `runsc`, and its sidecars remain under the ignored `.dev/` directory; only the builder image and ordinary application images use the current user's Podman storage. Podman, podman-compose, Git, curl, and the host user-namespace helpers must already be installed.

The prerequisites are explicit Make dependencies: `dev-up` depends on `images` and `dev-gvisor`; `images` depends on configuration plus both image targets. Run `make help` for the available setup, quality, image, and runtime targets.

The controller is an ordinary background process listening on `$XDG_RUNTIME_DIR/locoder-podman.sock`; it needs neither administrator access nor a project systemd unit. Patched runsc asks the current user's existing systemd manager to create delegated cgroup scopes. Stop both the stack and controller with:

```sh
make dev-down
```

Do not replace the patched runtime with `--ignore-cgroups`. That workaround starts stock runsc rootlessly but discards the per-sandbox resource limits this application relies on. `scripts/preflight.sh` reads the running sandbox's cgroup files and refuses startup unless the 256 MiB memory, one-CPU, and 64-PID limits are present.

## Rootless Linux deployment

Rootless Podman is the only supported deployment model. Ordinary Compose services use the current user's normal runtime, while API-created sandboxes go through the separate runsc-default socket managed by [gvisor-controller.sh](scripts/gvisor-controller.sh). Podman remote does not support selecting `--runtime`; controllers inspect every created container and refuse to start it unless its recorded OCI runtime is runsc.

`configure.py` generates random PostgreSQL and Valkey credentials and rootless socket/runtime paths; it refuses to overwrite `.env`. It defaults to loopback-only HTTP at `http://localhost:8080` with `COOKIE_SECURE=false`. For access from other machines, put an HTTPS reverse proxy in front of the loopback listener, set the exact `PUBLIC_ORIGIN`, and set `COOKIE_SECURE=true`. The proxy must support WebSockets. Do not expose the database, Valkey, or the Podman socket. `PORT` changes the loopback listener; `PUBLIC_ORIGIN` must match it.

With SSH forwarding, use the same loopback hostname in the browser and `PUBLIC_ORIGIN`: `http://localhost:8080` and `http://127.0.0.1:8080` are different origins. For the latter, set `PUBLIC_ORIGIN=http://127.0.0.1:8080` in `.env` and recreate `api` and `editor` before signing in.

`SANDBOX_RUNTIME` is an absolute path on the Podman server host. The installer verifies the pinned Git tag, uses a digest-pinned builder image, and installs `runsc` and its required sidecar directory under `.dev/`. The sandbox preflight invokes `dmesg` inside runsc, verifies gVisor identification, and checks its live cgroup limits. SELinux labeling is disabled on these containers because runsc rejects SELinux OCI labels; the other sandbox restrictions remain mandatory.

Bootstrap an administrator without putting the password in shell history or process arguments:

```sh
read -rsp 'New admin password (12+ characters): ' locoder_password; echo
printf '%s\n' "$locoder_password" | podman compose exec -T api api bootstrap-admin admin
unset locoder_password
```

Sign in, open **Authoring**, create users, and create/publish a problem. Authoring includes a JSON definition editor and sanitized Markdown preview. Three original sample definitions are in [samples](samples); publish them using:

```sh
PUBLIC_ORIGIN=http://localhost:8080 python3 scripts/seed.py
```

Run the command on the development host, using the same origin configured in `.env`. For an SSH setup configured with `PUBLIC_ORIGIN=http://127.0.0.1:8080`, use `PUBLIC_ORIGIN=http://127.0.0.1:8080 python3 scripts/seed.py` instead.

No public registration is provided. Changing a draft never changes a published version. Each submission remains associated with the version it used.

## Contract and behavior

Types are `"int"` (signed 32-bit), `"bool"`, `"string"`, or recursively nested `{"array": TYPE}`. Nesting is limited to eight levels. Parameter and method names must be safe identifiers in both languages. Arguments are JSON lists in parameter order. Comparisons are structural, with array order preserved. Floats, nulls, mutation-only methods, custom checkers, and user packages are unsupported.

The editor's single **Run** action executes the complete published test set. Results include an ordered status for every visible and hidden case; hidden inputs, expected answers, returned values, and logs never enter ordinary responses. Compilation diagnostics are visible because compilation receives no tests. The API retains its `run` mode for older clients and custom JSON cases, such as `[{"args":[[1,2,3]],"expected":[2]}]`; omit `expected` to inspect output without a correctness verdict. The result protocol is separate from the user's stdout/stderr and is treated as untrusted by the worker.

The browser supports search, difficulty/tag filters, resizable statement/editor panes, language switching, local drafts keyed by user/version/language, custom cases, result polling, and submission history. Monaco loads only on problem pages. `monaco-languageclient` connects to clangd or Pyright through one-use authenticated tickets. The server restricts LSP methods and workspace URIs, limits sessions to two per user/eight globally, and closes sessions after five minutes without client activity. A one-hour container lifetime bounds controller-crash leakage; reconnect starts a fresh session. Editing and submission remain available when semantic capacity is unavailable.

## Services and security boundaries

- `api`: Argon2id login, HttpOnly/SameSite cookies, origin + CSRF checks, role/ownership enforcement, immutable publishing, transactional submission creation and outbox dispatch, and result persistence. It has **no Podman socket**.
- `worker`: consumes a versioned Valkey job, reads its source and tests from PostgreSQL, and controls fresh runsc sandboxes. One submission per worker by default; set `WORKER_CONCURRENCY` or scale with `podman compose up -d --scale worker=2`.
- `editor`: Valkey-ticket authentication and isolated LSP containers; no PostgreSQL credentials.
- PostgreSQL and Valkey AOF persist in named volumes. Internal networks separate database and queue traffic. Nginx routes same-origin traffic and rate-limits login requests.

**Podman controller access is user-engine authority.** The worker/editor containers can control the dedicated rootless engine. Use a dedicated production user and trust the controller code. Submitted programs never receive the socket, credentials, expected answers, or other cases. Sandboxes have no networking, UID 65534, read-only roots, dropped capabilities, no-new-privileges, bounded tmpfs, CPU/memory/PID/file/output limits, and controller wall deadlines. Podman's container timeout also bounds an orphan after a worker crash.

Defaults: 2 seconds/256 MiB/output 1 MiB per case; C++ compilation 30 seconds/1 GiB. Publishing accepts administrator limits up to `MAX_TIME_MS` (10000) and `MAX_MEMORY_MIB` (1024); output ceiling is 1 MiB. Programs receive a fresh sandbox for each case; C++ artifacts transfer through the trusted controller, not a shared submission directory. The Podman backend exposes create/start/collect/kill/cleanup for a future Kubernetes implementation; no Kubernetes manifests or execution are included.

## Durability and operations

Submission source, custom cases, metadata, and the outbox record commit in one PostgreSQL transaction. `Idempotency-Key` is required and bound to user plus request hash; conflicting reuse returns 409. Published versions retain their complete immutable test set in PostgreSQL, while their public JSON contains visible tests only. The dispatcher delivers Valkey Stream jobs at least once. Workers use renewable 30-second leases, attempt tokens, pending-message reclaim, and an atomic Lua completion/event-publication/job-ack operation. Stale tokens cannot publish results. The API acknowledges result events only after committing and tolerates completion-before-start delivery. Accepted final results are immutable at the consumer boundary.

A database reconciliation pass republishes still-incomplete submissions after **one hour**, with a new durable generation token. Old-generation events cannot overwrite them. This recovers Valkey loss even when all stream state disappears. Recorded attempt counts carry into the new generation; infrastructure errors get up to three execution attempts. If Valkey loses an unpersisted started event, that attempt cannot be counted durably. The one-hour window exceeds the maximum supported 200-case workload. Pending recovery after an ordinary worker crash starts after 35 seconds.

Only acknowledged messages are removed from streams. Terminal submission, source, and outbox records are retained; plan a database retention policy before indefinite use. Do not trim pending streams or enable Valkey eviction. Valkey AOF `everysec` can lose a second of queue state; PostgreSQL is the durable authority.

The API exposes `/healthz`, `/readyz` (database/Valkey), and authenticated administrator `/metrics` (queue/status counts, verdict counts, execution duration totals, active editors). Worker/editor processes refuse startup when preflight fails. JSON service logs omit source, test content, and runtime logs. The ingress access log is disabled to avoid logging WebSocket tickets.

For consistent backups and restores, use the same rootless user and project:

```sh
scripts/backup.sh "$HOME/backups/locoder-2026-09-18"
# In a fresh checkout with no .env and no locoder_* volumes:
scripts/restore.sh "$HOME/backups/locoder-2026-09-18"
make dev-up
```

Backups stop the project and export the PostgreSQL and Valkey volumes together with credentials and configuration, then restart it. Protect backups as secrets. Restore refuses to overwrite existing volumes. Verify login, a known solve, and queued submission reconciliation after restoring. Container images should be archived alongside a production backup (`podman save`); locally built images have no registry digest until pushed. Base images and application dependency lockfiles are pinned; `pin-images.py` records immutable app/toolchain image IDs in `.env` for the selected Podman engine. Re-run it after an intentional rebuild; OS package repositories should be mirrored or the built images archived for byte-for-byte reproduction.

## Development and verification

### Cached container builds

Podman builds retain Cargo and npm downloads locally and arrange independent application stages so Podman can build them concurrently. Ordinary local builds need no cache configuration:

```sh
make images BUILD_JOBS=2
```

`make app-image` and `make toolchain-image` build the images separately. `make images` remains the compatibility target: it builds both images and records their immutable local IDs in `.env`. Override `PODMAN`, `APP_IMAGE`, or `TOOLCHAIN_IMAGE` when using a different engine command or image tags.

CI can also import and export intermediate layers through an OCI registry. Authenticate Podman to the registry first, then pass a repository prefix:

```sh
podman login registry.example.com
make images \
  CACHE_FROM_REPO=registry.example.com/team/locoder-cache \
  CACHE_TO_REPO=registry.example.com/team/locoder-cache
```

This uses the `app` and `toolchain` repositories below that prefix. Only trusted branches should set `CACHE_TO_REPO`; untrusted jobs should set `CACHE_FROM_REPO` alone. Leave both variables unset for normal local builds. Cache repositories must be private because intermediate layers can contain source files, and their registry retention policy should remove accumulated cache tags. These flags target the installed Podman 5.8 feature set; CI credentials and provider-specific workflow configuration remain external to this repository.

Run `make check` as a separate static-analysis gate before building images; image construction does not repeat that work.

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

Versioned REST documentation is [openapi.json](openapi.json), also served at `/api/v1/openapi.json`; generated frontend types are checked in. [tests/api_acceptance.py](tests/api_acceptance.py) and [browser tests](tests/browser) use a disposable administrator `admin` / `Integration-password-123`; never seed these credentials in production.

Set `DATABASE_URL` and `VALKEY_URL` to isolated services for opt-in integration tests. Run `cargo test --lib -- --ignored --test-threads=1` for Valkey/database recovery. `python3 tests/api_acceptance.py` and `npm test --prefix web` default to `http://127.0.0.1:18080` (`TEST_ORIGIN` overrides it).

On a working gVisor host, run `cargo test --test sandbox_acceptance -- --ignored` with `SANDBOX_RUNTIME`/`TOOLCHAIN_IMAGE` set and controller access to Podman, then `TEST_RUNNER=1 npm test --prefix web` against the full deployment. These checks are deliberately gated rather than using a weaker runtime.

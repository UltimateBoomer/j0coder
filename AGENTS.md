# Project overview

j0coder is a self-hosted coding practice platform for C++, Python, Java, and
Kotlin. It uses a Svelte/Monaco frontend, a Rust/Axum API, PostgreSQL, Valkey
Streams, and gVisor sandboxes. Published problem versions are immutable; hidden
tests stay on the server.

- `crates/platform/`: API, worker, editor, catalog controller, language harnesses,
  and Rust tests.
- `web/src/`: frontend; `tests/browser/`: Playwright tests.
- `migrations/`: PostgreSQL migrations; `samples/`: example problem definitions.
- `scripts/` and `deploy/`: setup, runtime, container, and deployment tooling.

# Documentation

- [README.md](README.md): product overview, production setup, and native development.
- [docs/architecture.md](docs/architecture.md): service boundaries and execution flows.
- [docs/security.md](docs/security.md): accounts, guest access, quotas, and operations.
- [docs/catalog.md](docs/catalog.md): Git catalog and problem authoring contracts.
- [docs/testing.md](docs/testing.md): disposable integration environments and acceptance tests.
- [docs/verification.md](docs/verification.md): recorded validation and its limitations.
- [deploy/helm/j0coder/README.md](deploy/helm/j0coder/README.md),
  [docs/local-kubernetes-fedora.md](docs/local-kubernetes-fedora.md), and
  [deploy/terraform/README.md](deploy/terraform/README.md): deployment guides.

# Checks and tests

Run commands from the repository root. CI uses stable Rust and Node.js 22.
[.github/workflows/ci.yml](.github/workflows/ci.yml) is the authoritative command
sequence; [.github/scripts/ci_changes.py](.github/scripts/ci_changes.py) selects
jobs by changed paths. Run checks relevant to the change before delivery, and
report any runtime checks that could not be performed.

## Standard checks

```sh
npm ci --prefix web
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
npm run check --prefix web
npm run build --prefix web
git diff --check
```

`make check` runs formatting, Clippy, and Svelte checks; `make test` runs the
ordinary Rust suite. Ignored integration tests require separate runtime setup.
If the host's sccache wrapper fails with `Operation not permitted`, run Cargo
with `RUSTC_WRAPPER=`.

For contract changes, run `make generate-contracts` and include updates to
`openapi.json`, `web/src/api.generated.ts`, and `problem.schema.json`. CI reruns
generation and requires no differences in those files.

## Browser tests with mocked APIs

Install Chromium with `(cd web && npx playwright install --with-deps chromium)`.
Start `npm run dev --prefix web` in another terminal, then run:

```sh
TEST_ORIGIN=http://127.0.0.1:8080 npm test --prefix web -- \
  editor-ui.spec.ts header-search.spec.ts problem-list.spec.ts \
  solution-sync.spec.ts rename.spec.ts security-settings.spec.ts constraints.spec.ts
```

## Integration and live acceptance

Use fresh, disposable PostgreSQL/Valkey services and the environment variables
in [docs/testing.md](docs/testing.md). Build the API and bootstrap the test admin
before running the CI integration job's Rust tests:

```sh
cargo test --locked --lib queue::integration -- --ignored --test-threads=1
SECURITY_TEST_DATABASE_URL="$DATABASE_URL" cargo test --locked --lib api::admission_tests -- --ignored
BOOTSTRAP_TEST_DATABASE_URL="$DATABASE_URL" cargo test --locked --test bootstrap -- --ignored
```

Follow the workflow's `integration` job for the complete sequence: security
migration/acceptance with guest browsing enabled, API restart with guest browsing
disabled and disposable Valkey reset, API acceptance, then `practice.spec.ts`
and `routing.spec.ts` with `--workers=1`. Security tests require
`SECURITY_TEST_DISPOSABLE=yes` and `j0coder-hardening-*` container names; CI uses
Docker, while local tests support Podman. Never point these tests at persistent
installation data. Full submission/LSP browser acceptance requires the gVisor
worker/editor stack from `make dev-up` and `TEST_RUNNER=1`; ordinary CI skips it.

## Scripts and deployment

Run Python selector/setup tests with `python3 -m unittest discover -s .github/scripts -p 'test_*.py'`
and `python3 -m unittest discover -s scripts -p test_setup.py`. For script/config
changes, use the workflow's shell syntax, Python compilation, and Compose
rendering checks. For Helm changes, run `make helm-check` and the workflow's
additional values/pull-secret renders. Container changes must pass the relevant
`deploy/App.Containerfile` or `deploy/Toolchain.Containerfile` build.

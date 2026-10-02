# Disposable integration environment

GitHub Actions CI runs on pull requests and pushes to `main`. It selects Rust
checks, frontend checks with mocked browser tests, PostgreSQL/Valkey API and
live browser acceptance tests, Helm checks, Compose/script checks, and the two
container builds from the changed files. Documentation-only changes report a
passing `CI result` without starting test jobs. A manual run executes all
checks. The browser test that executes user code under gVisor still requires
the complete rootless development environment described below.

Use a Podman engine separate from production. These credentials are intentionally public test fixtures. The script refuses pre-existing named containers rather than deleting them.

Stop the native development stack with `make dev-down` before starting these disposable dependencies; they use the same host ports.

For a complete rootless development environment, including live gVisor runners, use `make dev-up`. PostgreSQL and Valkey run in Compose; trusted services run natively in tmux. A background user-owned Podman API provides the sandbox runtime. `make dev-down` stops them. The preflight rejects a runtime that does not enforce the configured cgroup limits.

```sh
scripts/test-services.sh
export DATABASE_URL=postgres://postgres:integration-only@127.0.0.1:15432/practice
export VALKEY_URL=redis://127.0.0.1:16379
export PUBLIC_ORIGIN=http://127.0.0.1:18080
export API_BIND=127.0.0.1:18080
export COOKIE_SECURE=false
export WEB_ADMIN_ENABLED=true
export ADMIN_BIND=127.0.0.1:18082
export ADMIN_ORIGIN=http://127.0.0.1:18082
export TEST_ADMIN_ORIGIN=http://127.0.0.1:18082
cargo build --locked
printf '%s\n' 'Integration-password-123' | target/debug/api bootstrap-admin admin
target/debug/api
```

In another shell with the same variables:

```sh
cargo test --locked
cargo test --lib -- --ignored --test-threads=1
python3 tests/api_acceptance.py
npm ci --prefix web
npm run build --prefix web
(cd web && npx playwright install chromium)
npm test --prefix web
```

The API test creates a regular user and a problem each run. The browser test creates another problem. The queue recovery test creates isolated database records and random Valkey test keys. Use disposable data.

For the full runner/browser test, deploy the same test accounts with the rootless gVisor configuration in the README and set `TEST_RUNNER=1`. The fixture wrapper and direct LSP checks deliberately exercise **repository-owned code only** under a conventional container; they must never become a path for running submissions.

When finished, stop the test API process, then remove only the test dependencies:

```sh
podman rm -f j0coder-test-pg j0coder-test-valkey
```

## Account security acceptance

`tests/security_acceptance.py` uses the host CLI and real HTTP, PostgreSQL and Valkey. It requires `SECURITY_TEST_DISPOSABLE=yes`, native `target/debug/api`, a test API on 18080/private API on 18082, `GUEST_BROWSING_ENABLED=true`, and disposable containers named `j0coder-hardening-pg` and `j0coder-hardening-valkey` (override with `TEST_PG_CONTAINER` / `TEST_VALKEY_CONTAINER`, retaining the prefix). Set `TEST_CONTAINER_ENGINE=podman` (default) or `docker`, and export `DATABASE_URL` and `VALKEY_URL` pointing only to those dependencies. It creates test accounts/problems and clears anonymous read and dedicated redemption limiter keys in that test Valkey database, so use a fresh disposable environment. It tests admin audience separation, guest restrictions, CSRF, owner isolation, preference validation, single-use concurrent invitations, submission/HTTP quota overrides, hashed sessions, reset/suspension, queued cancellation, recovery under exhausted quotas and redemption spam limits.

The mocked `security-settings.spec.ts` covers guest-local code/preferences and absence of protected API requests, account editor defaults and semantic retry cancellation. These browser checks do not execute a real language server or sandbox. Terraform validation/mocked tests are documented separately in `deploy/terraform/README.md`.

Implementation verification on 2026-10-02: the Rust suite and Clippy passed, including real PostgreSQL/Valkey queue recovery and one-connection admission checks. The legacy-data security migration, nine security acceptance tests, six legacy API acceptance tests, 19 mocked browser tests and three enabled live browser tests passed. The full sandbox/semantic browser scenario remained skipped without `TEST_RUNNER=1`. Svelte checks/build, setup tests, Terraform validation/four mocked tests, Helm rendering, Compose rendering and Nginx configuration checks passed. Hosted CI, a new full gVisor/editor deployment, cloud provisioning and disaster recovery were not validated by these checks.

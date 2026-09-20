# Disposable integration environment

Use a Podman engine separate from production. These credentials are intentionally public test fixtures. The script refuses pre-existing named containers rather than deleting them.

For a complete rootless development deployment, including live gVisor runners, use `make dev-up`. It uses a project-local patched runsc and a background user-owned Podman API rather than a systemd unit. `make dev-down` stops it. The preflight rejects a runtime that does not enforce the configured cgroup limits.

```sh
scripts/test-services.sh
export DATABASE_URL=postgres://postgres:integration-only@127.0.0.1:15432/practice
export VALKEY_URL=redis://127.0.0.1:16379
export PUBLIC_ORIGIN=http://127.0.0.1:18080
export API_BIND=127.0.0.1:18080
export COOKIE_SECURE=false
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
podman rm -f locoder-test-pg locoder-test-valkey
```

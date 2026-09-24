# Verification record

Development host: Linux x86-64, Podman 5.8.4, podman-compose 1.6.0, cgroup v2, rootless crun, Rust 1.97.1, Node 22.23.1. Checks performed on 2026-09-18/19 (America/Toronto / UTC).

## Verified

- Rust workspace builds; formatter and strict Clippy checks pass.
- Svelte/TypeScript checks and production Vite build pass. Monaco is lazy-loaded. Its VS Code compatibility dependency produces large editor-only chunks; the initial problem-list bundle does not need them.
- Frontend dependency audit reports zero known vulnerabilities at verification time.
- Podman toolchain image builds: Clang/clangd 16.0.6, Python 3.11.2, Pyright 1.1.407; image size about 779 MB.
- Complete multistage Podman application image builds, including static frontend and all three Rust executables.
- Compose configuration validates with podman-compose.
- Typed contract tests cover nested/empty arrays, Unicode strings, int32 boundaries, booleans versus integers, invalid arguments and identifiers, original sample validity, and wrapper absence of test data.
- Repository-owned fixtures pass all 11 sample cases in each language (22 case executions). These ran in a bounded crun container and establish wrapper correctness only, **not** gVisor security.
- Real PostgreSQL/Valkey tests cover duplicate acquisition, expired lease takeover, stale completion rejection, atomic result publication/acknowledgment, database generation reconciliation, and completion-before-start event ordering.
- Problem versions, complete test bundles, user source, and custom cases commit atomically in PostgreSQL. Fresh-database API acceptance and worker payload-selection integration checks pass.
- The actual worker rejects a crun-backed remote Podman socket before container start. A dedicated runsc-default Podman service is supplied because the remote CLI cannot accept `--runtime`.
- The project-local gVisor `release-20260914.0` build runs through a user-owned Podman API with no installed systemd unit. Preflight inspection confirms its 256 MiB memory, one-CPU, and 64-PID cgroup limits. The live acceptance suite passes parallel C++/Python judging, accepted/wrong-answer/compilation/time/output/runtime/memory verdicts, hidden-case redaction, socket/credential isolation, and blocked networking.
- Packaged clangd and Pyright return semantic completion lists and diagnostics for trusted fixture text (4/85 completion items, one diagnostic each); this verifies the installed servers, not the isolated WebSocket integration.
- Real API tests cover local login, CSRF and origin checks, role/ownership checks, hidden-test redaction, filters, idempotency/conflicts, typed custom-case rejection, draft isolation, and immutable publication.
- Chromium acceptance covers admin publishing, Markdown script/event-handler sanitization, Monaco initialization, C++/Python switching, and continued editing with an unavailable semantic service. Screenshot inspected at `/tmp/practice-editor.png` during implementation.

## Deployment gates still required

Stock gVisor `release-20260914.0` fails with rootless Podman because runsc contacts the system service manager. Its `ignore-cgroups` workaround launches successfully but inspection showed that memory, CPU, and PID limits were not applied, so it remains forbidden. The verified project-local build applies the patches in `deploy/gvisor/`, uses the per-user systemd manager, handles hosts that do not delegate cpuset, and still enforces requested CPU, memory, and PID limits. The development preflight inspects those live sandbox cgroup limits on every startup.

Consequently, **the following are not claimed as verified**:

- Worker-crash cleanup in a live gVisor execution.
- Two live runner processes concurrently judging submissions and mixed-language dispatch.
- clangd/Pyright semantic completion and diagnostics through the complete isolated WebSocket path, reconnection, language changes, and five-minute idle cleanup.
- Full browser solve acceptance with real runners (`TEST_RUNNER=1`).
- End-to-end failure injection involving abrupt worker termination and actual Valkey or PostgreSQL restart. Protocol-level fencing/reconciliation tests do pass.
- A cold backup/restore drill on the deployed project, or a clean rootless production installation following the setup guide.

## Resource observations

Before database-backed payloads were introduced, an idle disposable PostgreSQL container measured **30.34 MB** and the legacy queue service measured **9.74 MB** (decimal units from `podman stats`). This predates the Valkey deployment and is not a Valkey measurement or production capacity guarantee. The packaged API measured **65.15 MB** after integration traffic. Active runner and active editor memory totals require measurement on the final gVisor host. Per-container configured ceilings are 256 MiB per default test, 1 GiB compilation, and 512 MiB per editor; these are limits, **not measured usage**.

## Remaining product limits

- Problem authoring uses a structured JSON editor with Markdown preview, not separate form controls for every interface/test field.
- Reconciliation after total Valkey loss has a one-hour maximum scheduling delay.
- Source and submission retention is manual; define a database retention policy before indefinite use.
- Local toolchain/app builds need image archival or registry digest publication for reproducible deployment; apt repository contents are not snapshot-pinned.
- Syntax/basic completion is available without LSP. Semantic service availability is explicitly displayed.

# Git-managed problem catalog

The catalog controller reads one private SSH Git repository. Deployment values seed settings only when `catalog_settings` is empty; later changes are made through the host CLI (`api manage catalog FILE`; see [security operations](security.md)). Configure `CATALOG_REPOSITORY_URL`, `CATALOG_STRATEGY` (`track_branch` or `pinned_commit`), `CATALOG_REVISION`, `CATALOG_POLL_INTERVAL_SECONDS`, and `CATALOG_ENABLED`. Mount an SSH deploy key and pinned `known_hosts` file using `CATALOG_SSH_KEY_PATH` and `CATALOG_KNOWN_HOSTS_PATH`.

The current source format is described in [Git-native problem authoring](git-problem-authoring-plan.md). Each `problems/<slug>/` directory contains `problem.yaml` with its statement, ordered hints, and visible and hidden case lists, plus an optional private reference. The sibling repository's `example-problem/` is a copyable template outside the published catalog. The controller resolves a Git commit, validates the complete release, and atomically reconciles immutable learner versions. A format-only migration preserves the learner version when the assembled problem is unchanged; reference-only edits do not change it. Hidden cases remain private.

Validate before merging a catalog change:

```sh
RUSTC_WRAPPER= cargo run --locked -p j0coder --bin catalog-validate -- ../j0coder-problems
RUSTC_WRAPPER= cargo run --locked -p j0coder --bin catalog-validate -- ../j0coder-problems --check-references
```

Reference checks require the same configured gVisor sandbox and toolchain image as learner submissions. The controller never executes catalog source. Legacy `catalog.json` releases are rejected. The manifest builder has been retired for the directory catalog.

Rust contracts generate OpenAPI, TypeScript, and the draft-07 editor schema. Run `make generate-contracts` after changing contracts. Update the catalog editor schema and `.j0coder-revision` together after delivering the application commit. Catalog CI validates against that exact full commit SHA; cross-field, byte-length, filesystem, and typed-case rules remain enforced by Rust.

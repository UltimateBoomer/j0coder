# Git-managed problem catalog

The catalog controller reads one private SSH Git repository. Deployment values seed settings only when `catalog_settings` is empty; later changes are made through the host CLI (`api manage catalog FILE`; see [security operations](security.md)). Configure `CATALOG_REPOSITORY_URL`, `CATALOG_STRATEGY` (`track_branch` or `pinned_commit`), `CATALOG_REVISION`, `CATALOG_POLL_INTERVAL_SECONDS`, and `CATALOG_ENABLED`. Mount an SSH deploy key and pinned `known_hosts` file using `CATALOG_SSH_KEY_PATH` and `CATALOG_KNOWN_HOSTS_PATH`.

The current source format is described in [Git-native problem authoring](git-problem-authoring-plan.md). Each `problems/<slug>/` directory contains `problem.yaml` with its statement, ordered hints, and visible and hidden case lists, plus an optional private reference. The sibling repository's `example-problem/` is a copyable template outside the published catalog. The controller resolves a Git commit, validates the complete release, and atomically reconciles immutable learner versions. A format-only migration preserves the learner version when the assembled problem is unchanged; reference-only edits do not change it. Hidden cases remain private.

Validate before merging a catalog change:

The Python quality and oracle scripts use `PyYAML` from `scripts/requirements-catalog.txt`.

```sh
RUSTC_WRAPPER= cargo run --locked -p j0coder --bin catalog-validate -- ../j0coder-problems
RUSTC_WRAPPER= cargo run --locked -p j0coder --bin catalog-validate -- ../j0coder-problems --check-references
python3 scripts/check-catalog-quality.py ../j0coder-problems
python3 scripts/catalog_oracles/check_004_036_and_101_110.py ../j0coder-problems
python3 scripts/catalog_oracles/check_037_071_and_111_120.py ../j0coder-problems
python3 scripts/catalog_oracles/check_072_099_and_121_125.py ../j0coder-problems
```

Reference checks require the same configured gVisor sandbox and toolchain image as learner submissions. The controller never executes catalog source. During transition it also accepts a legacy `catalog.json` release; mixed releases are rejected. The manifest builder has been retired for the directory catalog.

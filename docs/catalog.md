# Git-managed problem catalog

The catalog controller reads one private SSH Git repository. Deployment values seed the database only when `catalog_settings` is empty; later changes are made in the administrator UI and survive restarts.

Set `CATALOG_REPOSITORY_URL`, `CATALOG_STRATEGY` (`track_branch` or `pinned_commit`), `CATALOG_REVISION`, `CATALOG_POLL_INTERVAL_SECONDS`, and `CATALOG_ENABLED`. Mount an SSH deploy key and a pinned OpenSSH `known_hosts` file using `CATALOG_SSH_KEY_PATH` and `CATALOG_KNOWN_HOSTS_PATH`. HTTPS and local repository URLs are rejected.

For native local development, keep the deploy key owned by your host account with mode `0600`. Set `CATALOG_SSH_KEY_PATH` and `CATALOG_KNOWN_HOSTS_PATH` to host paths in `.env`; the catalog controller reads them as your user. After changing those settings, restart its tmux window with `tmux -L locoder-dev respawn-window -k -t locoder:catalog-controller`. The full Compose deployment uses Podman's `keep-id` user namespace to map the host account to image UID 65534.

The repository root must contain `catalog.json`:

```json
{
  "schema": 1,
  "problems": [
    {
      "key": "arrays/two-sum",
      "path": "problems/two-sum.json",
      "sha256": "<SHA-256 of the exact artifact bytes>"
    }
  ]
}
```

Each problem artifact must declare `schema: 3` and a `function` or `data_structure` interface. The manifest remains schema 1.

For the sibling `code-practice-problems` checkout, update exact-byte checksums and validate the release before publishing:

```sh
python3 scripts/build-catalog-manifest.py ../code-practice-problems --expect 125
cargo run --locked -p locoder --bin catalog-validate -- ../code-practice-problems
python3 scripts/check-catalog-quality.py ../code-practice-problems
python3 scripts/catalog_oracles/check_004_036_and_101_110.py ../code-practice-problems
python3 scripts/catalog_oracles/check_037_071_and_111_120.py ../code-practice-problems
python3 scripts/catalog_oracles/check_072_099_and_121_125.py ../code-practice-problems
```

The validator stages a copy of the working tree without `.git`, checks every schema-3 artifact, then checks the complete release. The quality check enforces the [curation plan](problem-curation-plan.md)'s final difficulty mix and minimum visible/hidden case counts for new problems. The oracle scripts recompute the expected results of the 121 new artifacts.

Every regular file in a release must be listed (apart from `catalog.json`). Keys and paths must be unique. Symlinks, traversal, extra files, checksum mismatches, and invalid problem contracts reject the complete release. The controller never runs repository code.

`track_branch` resolves the configured branch on each poll. `pinned_commit` requires a full 40-character commit hash and changes only when an administrator changes that hash. Successful changes are automatically published as immutable versions; unchanged artifacts reuse their versions. Removing a key unlists the problem while retaining its history and provenance.

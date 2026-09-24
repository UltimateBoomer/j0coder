# Git-managed problem catalog

The catalog controller reads one private SSH Git repository. Deployment values seed the database only when `catalog_settings` is empty; later changes are made in the administrator UI and survive restarts.

Set `CATALOG_REPOSITORY_URL`, `CATALOG_STRATEGY` (`track_branch` or `pinned_commit`), `CATALOG_REVISION`, `CATALOG_POLL_INTERVAL_SECONDS`, and `CATALOG_ENABLED`. Mount an SSH deploy key and a pinned OpenSSH `known_hosts` file using `CATALOG_SSH_KEY_PATH` and `CATALOG_KNOWN_HOSTS_PATH`. HTTPS and local repository URLs are rejected.

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

Every regular file in a release must be listed (apart from `catalog.json`). Keys and paths must be unique. Symlinks, traversal, extra files, checksum mismatches, and invalid problem contracts reject the complete release. The controller never runs repository code.

`track_branch` resolves the configured branch on each poll. `pinned_commit` requires a full 40-character commit hash and changes only when an administrator changes that hash. Successful changes are automatically published as immutable versions; unchanged artifacts reuse their versions. Removing a key unlists the problem while retaining its history and provenance.

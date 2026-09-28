# Git-native problem authoring

The catalog is edited as ordinary files in `locoder-problems`. There is no authoring CLI or application-held Git credential. A copyable, unpublished `example-problem/` directory shows the full format.

## Layout

```text
problems/between-the-markers/
  problem.yaml
  statement.md
  hints/01.md
  tests/visible/01.json
  tests/hidden/01.yaml
  reference.py        # optional; reference.cpp is also supported
```

`problem.yaml` has source `schema: 1`, a stable `key`, and the schema-3 problem's metadata, interface, and limits. The loader finds one directory per problem directly under `problems/`; it discovers other files by path and sorts hints and cases lexicographically. Tests contain the existing case shape without `hidden`; their directory supplies visibility. YAML and JSON cases have identical typed validation. A problem may have zero hints and at most one reference source. `example-problem/` is outside `problems/` and never published.

Each interface parameter can have an optional `constraints` Markdown string. The learner UI and admin preview display these strings in a separate Constraints section, ordered by the function, constructor, or method interface. A present string must be nonblank and at most 1,000 UTF-8 bytes. These constraints describe valid inputs; they do not add machine-enforced checks to cases or submissions.

YAML is restricted to JSON-compatible values. Duplicate keys, custom tags, aliases, multiple documents, non-string map keys, and values outside JSON's number range are invalid. The Rust catalog validator also rejects unknown files, symlinks, duplicate keys and titles, invalid paths, and invalid typed cases.

## Publication and references

The controller accepts the old `catalog.json` release during deployment transition, but a release cannot mix formats. Directory releases use the assembled schema-3 problem as the learner version identity. The transition compares semantic content with existing drafts; an unchanged problem keeps its version and saved solutions. Statements, hints, interface, or case changes publish a new immutable version. Reference-only changes create or bind a private reference revision without changing the learner version.

References are optional for migrated problems and never appear in learner API responses or public version JSON. `catalog-validate --check-references` runs every present reference against visible and hidden cases using the same gVisor sandbox, wrappers, limits, and comparison rules as submissions. The controller does not execute repository code. Catalog CI must run this check before changes reach the tracked branch; pinned commits need the same check before selection. A future arbitrary-input feature can execute the bound reference revision to obtain typed expected outputs without exposing source.

Hints are published in filename order and revealed one at a time in the practice UI through sanitized Markdown.

## Authoring and verification

Copy `example-problem/` to `problems/<slug>/`, edit its key, metadata, statement, cases, and optional reference, then run:

```sh
RUSTC_WRAPPER= cargo run --locked -p locoder --bin catalog-validate -- ../locoder-problems
RUSTC_WRAPPER= cargo run --locked -p locoder --bin catalog-validate -- ../locoder-problems --check-references
python3 scripts/check-catalog-quality.py ../locoder-problems
```

The second command needs the configured gVisor sandbox and toolchain image. Author changes through a normal Git branch and pull request. Catalog CI must pass before merge.

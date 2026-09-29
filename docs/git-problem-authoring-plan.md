# Git-native problem authoring

The catalog is edited as ordinary files in `locoder-problems`. There is no authoring CLI or application-held Git credential. A copyable, unpublished `example-problem/` directory shows the full format.

## Layout

```text
problems/between-the-markers/
  problem.yaml        # metadata, statement, hints, interface, visible and hidden cases
  reference.py        # optional; reference.cpp is also supported
```

`problem.yaml` has source `schema: 1`, a stable `key`, integer `difficulty` from 1 to 5, a Markdown `statement`, an ordered list of Markdown `hints`, `tests.visible` and `tests.hidden` case lists, and the problem's metadata, interface, and limits. Use a YAML literal block (`|`) for paragraph-form statements and multiline hints. The loader derives the easy, medium, or hard band from the integer when assembling the schema-3 problem. It finds one directory per problem directly under `problems/`. Tests use the existing case shape without `hidden`; their list supplies visibility. Cases retain list order, with visible cases before hidden cases. A problem may have zero hints and at most one reference source. `example-problem/` is outside `problems/` and never published.

The older `statement.md`, ordered `hints/*.md`, string `difficulty` with optional `difficulty_score`, and case files under `tests/visible/` or `tests/hidden/` still load during migration. A statement, hints, or tests must come from one location, not both.

Each interface parameter can have an optional `constraints` Markdown string. The learner UI and admin preview display these strings in a separate Constraints section, ordered by the function, constructor, or method interface. A present string must be nonblank and at most 1,000 UTF-8 bytes. These constraints describe valid inputs; they do not add machine-enforced checks to cases or submissions.

YAML is restricted to JSON-compatible values. Duplicate keys, custom tags, aliases, multiple documents, non-string map keys, and values outside JSON's number range are invalid. The Rust catalog validator also rejects unknown files, symlinks, duplicate keys and titles, invalid paths, and invalid typed cases.

## Publication and references

The controller accepts the old `catalog.json` release during deployment transition, but a release cannot mix formats. Directory releases use the assembled schema-3 problem as the learner version identity. The transition compares semantic content with existing drafts; an unchanged problem keeps its version and saved solutions. Statements, hints, interface, or case changes publish a new immutable version. Reference-only changes create or bind a private reference revision without changing the learner version.

References are optional for migrated problems and never appear in learner API responses or public version JSON. `catalog-validate --check-references` runs every present reference against visible and hidden cases using the same gVisor sandbox, wrappers, limits, and comparison rules as submissions. The controller does not execute repository code. Catalog CI must run this check before changes reach the tracked branch; pinned commits need the same check before selection. A future arbitrary-input feature can execute the bound reference revision to obtain typed expected outputs without exposing source.

Hints are published in list order and revealed one at a time in the practice UI through sanitized Markdown. Legacy hint files retain filename order.

## Authoring and verification

Copy `example-problem/` to `problems/<slug>/`, edit its key, metadata, statement, cases, and optional reference, then run:

```sh
RUSTC_WRAPPER= cargo run --locked -p locoder --bin catalog-validate -- ../locoder-problems
RUSTC_WRAPPER= cargo run --locked -p locoder --bin catalog-validate -- ../locoder-problems --check-references
python3 scripts/check-catalog-quality.py ../locoder-problems
```

The second command needs the configured gVisor sandbox and toolchain image. Author changes through a normal Git branch and pull request. Catalog CI must pass before merge.

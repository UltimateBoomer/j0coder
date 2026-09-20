CREATE TYPE catalog_strategy AS ENUM ('track_branch', 'pinned_commit');
CREATE TABLE catalog_settings(
 singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton), repository_url text NOT NULL,
 strategy catalog_strategy NOT NULL, revision text NOT NULL,
 poll_interval_seconds integer NOT NULL CHECK (poll_interval_seconds BETWEEN 10 AND 86400),
 enabled boolean NOT NULL DEFAULT true, generation bigint NOT NULL DEFAULT 1,
 updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE catalog_runs(
 id uuid PRIMARY KEY, settings_generation bigint NOT NULL, repository_url text NOT NULL,
 requested_revision text NOT NULL, resolved_commit text, manifest_checksum text,
 result text NOT NULL CHECK (result IN ('running','applied','unchanged','failed','locked')),
 created_count integer NOT NULL DEFAULT 0, changed_count integer NOT NULL DEFAULT 0,
 unchanged_count integer NOT NULL DEFAULT 0, removed_count integer NOT NULL DEFAULT 0,
 diagnostics text, started_at timestamptz NOT NULL DEFAULT now(), finished_at timestamptz
);
CREATE INDEX catalog_runs_recent ON catalog_runs(started_at DESC);
CREATE TABLE problem_imports(
 catalog_key text PRIMARY KEY CHECK (catalog_key ~ '^[a-z0-9][a-z0-9._:/-]{2,199}$'),
 problem_id uuid UNIQUE NOT NULL REFERENCES problems(id), artifact_path text NOT NULL,
 artifact_hash text NOT NULL, repository_url text NOT NULL, resolved_commit text NOT NULL,
 manifest_checksum text NOT NULL, updated_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE versions ADD COLUMN catalog_provenance jsonb;

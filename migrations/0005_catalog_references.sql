ALTER TABLE catalog_runs RENAME COLUMN manifest_checksum TO source_checksum;
ALTER TABLE problem_imports RENAME COLUMN manifest_checksum TO source_checksum;

CREATE TABLE catalog_reference_revisions (
  id uuid PRIMARY KEY,
  problem_id uuid NOT NULL REFERENCES problems(id),
  language text NOT NULL CHECK (language IN ('python', 'cpp')),
  source text NOT NULL,
  source_hash text NOT NULL,
  repository_url text NOT NULL,
  resolved_commit text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (problem_id, language, source_hash)
);
CREATE FUNCTION reject_reference_revision_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'Catalog reference revisions are immutable';
END;
$$;
CREATE TRIGGER catalog_reference_revisions_immutable
BEFORE UPDATE OR DELETE ON catalog_reference_revisions
FOR EACH ROW EXECUTE FUNCTION reject_reference_revision_change();
CREATE TABLE catalog_reference_bindings (
  version_id uuid PRIMARY KEY REFERENCES versions(id),
  revision_id uuid NOT NULL REFERENCES catalog_reference_revisions(id),
  updated_at timestamptz NOT NULL DEFAULT now()
);

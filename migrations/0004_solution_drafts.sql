CREATE TABLE solution_drafts (
 user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 version_id uuid NOT NULL REFERENCES versions(id),
 language text NOT NULL CHECK (language IN ('cpp', 'python')),
 source text NOT NULL CHECK (octet_length(source) <= 100000),
 updated_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (user_id, version_id, language)
);

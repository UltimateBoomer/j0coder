ALTER TABLE solution_drafts DROP CONSTRAINT solution_drafts_language_check;
ALTER TABLE solution_drafts ADD CONSTRAINT solution_drafts_language_check CHECK (language IN ('cpp', 'python', 'java', 'kotlin'));
ALTER TABLE catalog_reference_revisions DROP CONSTRAINT catalog_reference_revisions_language_check;
ALTER TABLE catalog_reference_revisions ADD CONSTRAINT catalog_reference_revisions_language_check CHECK (language IN ('cpp', 'python', 'java', 'kotlin'));

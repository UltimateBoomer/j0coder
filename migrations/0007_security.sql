-- Bearer tokens are no longer stored in plaintext. Existing browsers must sign in again.
DELETE FROM sessions;
ALTER TABLE users ADD COLUMN suspended boolean NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN auth_generation bigint NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN audience text NOT NULL DEFAULT 'public' CHECK (audience IN ('public','admin'));
ALTER TABLE sessions ADD COLUMN generation bigint NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN created_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE sessions ADD COLUMN last_seen timestamptz NOT NULL DEFAULT now();
ALTER TABLE sessions ADD COLUMN authenticated_at timestamptz NOT NULL DEFAULT now();
CREATE INDEX sessions_user ON sessions(user_id,created_at);
CREATE TABLE account_tokens (
 token_hash text PRIMARY KEY, purpose text NOT NULL CHECK (purpose IN ('invite','reset')),
 user_id uuid REFERENCES users(id), expires timestamptz NOT NULL,
 CHECK ((purpose='invite' AND user_id IS NULL) OR (purpose='reset' AND user_id IS NOT NULL))
);
CREATE INDEX account_tokens_expiry ON account_tokens(expires);
CREATE TABLE user_policies (
 user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
 api_rate integer CHECK (api_rate BETWEEN 1 AND 10000),
 save_rate integer CHECK (save_rate BETWEEN 1 AND 10000),
 submissions_minute integer CHECK (submissions_minute BETWEEN 1 AND 1000),
 submissions_day integer CHECK (submissions_day BETWEEN 1 AND 100000),
 pending integer CHECK (pending BETWEEN 1 AND 100),
 ticket_rate integer CHECK (ticket_rate BETWEEN 1 AND 1000),
 editor_sessions integer CHECK (editor_sessions BETWEEN 1 AND 16),
 editor_messages integer CHECK (editor_messages BETWEEN 1 AND 1000),
 editor_bytes integer CHECK (editor_bytes BETWEEN 1024 AND 16777216)
);
CREATE TABLE user_preferences (
 user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
 theme text NOT NULL DEFAULT 'system' CHECK (theme IN ('system','light','dark')),
 default_language text NOT NULL DEFAULT 'cpp' CHECK (default_language IN ('cpp','python','java','kotlin')),
 semantic_completion boolean NOT NULL DEFAULT true,
 font_size integer NOT NULL DEFAULT 14 CHECK (font_size BETWEEN 10 AND 24),
 tab_width integer NOT NULL DEFAULT 4 CHECK (tab_width IN (2,4,8)),
 word_wrap boolean NOT NULL DEFAULT false, minimap boolean NOT NULL DEFAULT false,
 blind_mode boolean NOT NULL DEFAULT false
);
CREATE TABLE security_audit (
 id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, occurred_at timestamptz NOT NULL DEFAULT now(),
 action text NOT NULL, user_id uuid REFERENCES users(id)
);

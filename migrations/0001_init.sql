CREATE TABLE users(id uuid PRIMARY KEY, username text UNIQUE NOT NULL, password text NOT NULL, admin boolean NOT NULL DEFAULT false);
CREATE TABLE sessions(id text PRIMARY KEY, user_id uuid NOT NULL REFERENCES users(id), csrf text NOT NULL, expires timestamptz NOT NULL DEFAULT now()+interval '7 days');
CREATE INDEX sessions_expiry ON sessions(expires);
CREATE TABLE problems(id uuid PRIMARY KEY, draft jsonb NOT NULL, current_version uuid);
CREATE TABLE versions(id uuid PRIMARY KEY, problem_id uuid NOT NULL REFERENCES problems(id), public jsonb NOT NULL, tests jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
ALTER TABLE problems ADD FOREIGN KEY(current_version) REFERENCES versions(id);
CREATE TABLE submissions(id uuid PRIMARY KEY,user_id uuid NOT NULL REFERENCES users(id),version_id uuid NOT NULL REFERENCES versions(id),idempotency_key text NOT NULL,request_hash text NOT NULL,source text NOT NULL,custom_cases jsonb,job jsonb NOT NULL,status text NOT NULL DEFAULT 'queued',attempt integer NOT NULL DEFAULT 0,token text,result jsonb,created_at timestamptz NOT NULL DEFAULT now(),updated_at timestamptz NOT NULL DEFAULT now(),UNIQUE(user_id,idempotency_key));
CREATE TABLE outbox(submission_id uuid PRIMARY KEY REFERENCES submissions(id),last_sent timestamptz);
CREATE INDEX submissions_owner ON submissions(user_id,created_at DESC);

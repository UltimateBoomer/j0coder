#!/usr/bin/env python3
"""Verify migration 0007 against legacy data in a disposable private schema."""
import os,pathlib,subprocess,uuid
assert os.environ.get('SECURITY_TEST_DISPOSABLE')=='yes'
ENGINE=os.environ.get('TEST_CONTAINER_ENGINE','podman')
assert ENGINE in ('podman','docker')
container=os.environ.get('TEST_PG_CONTAINER','j0coder-hardening-pg')
assert container.startswith('j0coder-hardening-')
schema='migration_'+uuid.uuid4().hex
user,problem,version,submission=(str(uuid.uuid4()) for _ in range(4))
legacy='\n'.join(p.read_text() for p in sorted(pathlib.Path('migrations').glob('*.sql')) if p.name<'0007_security.sql')
script=f'''CREATE SCHEMA {schema}; SET search_path TO {schema};
{legacy}
INSERT INTO users VALUES('{user}','existing','old-argon2-hash',true);
INSERT INTO sessions(id,user_id,csrf) VALUES('legacy-plaintext-token','{user}','csrf');
INSERT INTO problems(id,draft) VALUES('{problem}','{{"draft":"preserved"}}');
INSERT INTO versions(id,problem_id,public,tests) VALUES('{version}','{problem}','{{"schema":3}}','[{{"hidden":true}}]');
UPDATE problems SET current_version='{version}';
INSERT INTO solution_drafts(user_id,version_id,language,source) VALUES('{user}','{version}','python','private old source');
INSERT INTO submissions(id,user_id,version_id,idempotency_key,request_hash,source,job) VALUES('{submission}','{user}','{version}','old','hash','submitted old source','{{}}');
INSERT INTO outbox(submission_id) VALUES('{submission}');
{pathlib.Path('migrations/0007_security.sql').read_text()}
DO $$ BEGIN
 IF (SELECT count(*) FROM sessions)<>0 THEN RAISE EXCEPTION 'legacy sessions survive'; END IF;
 IF NOT EXISTS(SELECT 1 FROM users WHERE id='{user}' AND password='old-argon2-hash' AND admin AND NOT suspended AND auth_generation=0) THEN RAISE EXCEPTION 'account changed'; END IF;
 IF NOT EXISTS(SELECT 1 FROM solution_drafts WHERE source='private old source') THEN RAISE EXCEPTION 'draft lost'; END IF;
 IF NOT EXISTS(SELECT 1 FROM submissions WHERE source='submitted old source') THEN RAISE EXCEPTION 'submission lost'; END IF;
 IF (SELECT count(*) FROM versions)<>1 OR (SELECT count(*) FROM problems)<>1 OR (SELECT count(*) FROM outbox)<>1 THEN RAISE EXCEPTION 'content lost'; END IF;
END $$;
DROP SCHEMA {schema} CASCADE;
'''
subprocess.run([ENGINE,'exec','-i',container,'psql','-U','postgres','-d','practice','-v','ON_ERROR_STOP=1','-q'],input=script,text=True,check=True,stdout=subprocess.DEVNULL)
print('Security migration preserves legacy data and invalidates sessions: PASS')

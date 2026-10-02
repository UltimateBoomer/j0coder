#!/usr/bin/env python3
"""Security acceptance against disposable PostgreSQL/Valkey and public/private APIs.
Never run against an existing installation. Requires SECURITY_TEST_DISPOSABLE=yes.
"""
import concurrent.futures, hashlib, http.cookiejar, json, os, pathlib, subprocess, unittest, urllib.error, urllib.request, uuid
PUBLIC=os.environ.get('TEST_ORIGIN','http://127.0.0.1:18080')
PRIVATE=os.environ.get('TEST_ADMIN_ORIGIN','http://127.0.0.1:18082')
ENGINE=os.environ.get('TEST_CONTAINER_ENGINE','podman')
assert ENGINE in ('podman','docker')
PG=os.environ.get('TEST_PG_CONTAINER','j0coder-hardening-pg')
VK=os.environ.get('TEST_VALKEY_CONTAINER','j0coder-hardening-valkey')
PASSWORD='Disposable-test-password-123'
def cli(*args,stdin=None):
 return subprocess.check_output(['target/debug/api','manage',*args],input=stdin,text=True).strip()
def sql(statement):
 return subprocess.check_output([ENGINE,'exec',PG,'psql','-U','postgres','-d','practice','-Atc',statement],text=True).strip()
def clear_guest_bucket():
 key='limit:ip:'+hashlib.sha256(b'127.0.0.1').hexdigest()+':read'
 subprocess.run([ENGINE,'exec',VK,'valkey-cli','DEL',key],check=True,stdout=subprocess.DEVNULL)
def clear_redemption_buckets():
 digest=hashlib.sha256(b'127.0.0.1').hexdigest()
 subprocess.run([ENGINE,'exec',VK,'valkey-cli','DEL','redeem:'+digest,'limit:ip:'+digest+':redeem'],check=True,stdout=subprocess.DEVNULL)
class Client:
 def __init__(self,origin=PUBLIC):
  self.origin=origin;self.jar=http.cookiejar.CookieJar();self.http=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.jar));self.csrf=''
 def request(self,path,method='GET',body=None,headers=None):
  clear_guest_bucket()
  req=urllib.request.Request(self.origin+'/api/v1'+path,method=method,data=json.dumps(body).encode() if body is not None else None,headers={'Origin':self.origin,'Content-Type':'application/json','X-CSRF-Token':self.csrf,**(headers or {})})
  try:r=self.http.open(req)
  except urllib.error.HTTPError as e:r=e
  with r:
   raw=r.read()
   try:data=json.loads(raw) if raw else None
   except ValueError:data=raw.decode()
   return r.status,data,dict(r.headers)
 def login(self,name):
  assert self.request('/session','POST',{'username':name,'password':PASSWORD})[0]==200
  self.csrf=self.request('/session')[1]['csrf']
class Security(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  assert os.environ.get('SECURITY_TEST_DISPOSABLE')=='yes','Disposable environment acknowledgement required'
  assert PG.startswith('j0coder-hardening-') and VK.startswith('j0coder-hardening-')
  suffix=uuid.uuid4().hex[:8];cls.names=['admin'+suffix,'alice'+suffix,'bob'+suffix]
  for i,name in enumerate(cls.names):cli('create-user',name,*(['admin'] if i==0 else []),stdin=PASSWORD+'\n')
  cls.admin=Client();cls.admin.login(cls.names[0]);cls.alice=Client();cls.alice.login(cls.names[1]);cls.bob=Client();cls.bob.login(cls.names[2])
  cls.private=Client(PRIVATE);cls.private.login(cls.names[0])
  cls.ids={name:sql("SELECT id FROM users WHERE username='"+name+"'") for name in cls.names}
  cls.problem=json.loads(cli('import','samples/1.json'))['id'];cls.version=json.loads(cli('publish',cls.problem))['version']
 def test_01_public_admin_boundary(self):
  self.assertFalse(self.admin.request('/session')[1]['admin'])
  for c in [Client(),self.admin,self.alice]:
   for path,method in [('/admin/problems','GET'),('/admin/users','POST'),('/admin/catalog','PUT')]:self.assertEqual(c.request(path,method,{} if method!='GET' else None)[0],404)
  self.assertTrue(self.private.request('/session')[1]['admin'])
  self.assertEqual(self.private.request('/admin/problems')[0],200)
  # A public token cannot authenticate on the private listener, or vice versa.
  cross=Client(PRIVATE);cross.jar=self.admin.jar;cross.http=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cross.jar))
  self.assertEqual(cross.request('/session')[0],401)
  self.assertEqual(self.private.request('/admin/problems/validate','POST',{}, {'X-CSRF-Token':'wrong'})[0],403)
  sql("UPDATE sessions SET authenticated_at=now()-interval '6 minutes' WHERE audience='admin'")
  self.assertEqual(self.private.request('/admin/problems/validate','POST',{})[0],403)
  self.assertEqual(self.private.request('/me/reauthenticate','POST',{'username':'','password':PASSWORD})[0],200)
 def test_02_guest_and_hidden_data(self):
  guest=Client();status,data,headers=guest.request('/problems/'+self.problem)
  self.assertEqual(status,200);self.assertEqual(headers['cache-control'],'no-store');self.assertTrue(all(not t['hidden'] for t in data['problem']['tests']))
  for path,method,body in [('/submissions','GET',None),('/editor-ticket','POST',{'language':'cpp'}),('/solutions/'+self.version+'/cpp','GET',None),('/me/preferences','GET',None),('/submissions','POST',{})]:self.assertEqual(guest.request(path,method,body)[0],401)
 def test_03_preferences_csrf_and_ownership(self):
  self.assertEqual(self.alice.request('/me/preferences','PATCH',{'theme':'dark'}, {'Origin':'https://evil.example'})[0],403)
  self.assertEqual(self.alice.request('/me/preferences','PATCH',{'theme':'dark'}, {'X-CSRF-Token':'wrong'})[0],403)
  for patch in [{'font_size':100},{'tab_width':3},{'admin':True},{'semantic_completion':'false'}]:self.assertEqual(self.alice.request('/me/preferences','PATCH',patch)[0],400)
  self.assertEqual(self.alice.request('/me/preferences','PATCH',{'theme':'dark','default_language':'python','semantic_completion':False})[0],200)
  self.assertEqual(self.bob.request('/me/preferences')[1]['theme'],'system')
  self.assertEqual(self.alice.request('/editor-ticket','POST',{'language':'cpp'})[0],403)
  path='/solutions/'+self.version+'/python'
  self.assertEqual(self.alice.request(path,'PUT',{'source':'private source'})[0],204)
  self.assertEqual(self.bob.request(path)[0],404)
 def test_04_single_use_invitation(self):
  token=cli('invite').split('#')[1]
  with concurrent.futures.ThreadPoolExecutor() as pool:
   results=list(pool.map(lambda name:Client().request('/register','POST',{'token':token,'username':name,'password':PASSWORD})[0],['new'+uuid.uuid4().hex[:8] for _ in range(2)]))
  self.assertEqual(sorted(results),[201,400])
  self.assertEqual(sql("SELECT count(*) FROM account_tokens WHERE token_hash='"+hashlib.sha256(token.encode()).hexdigest()+"'"),'0')
 def test_05_submission_limits_and_isolation(self):
  cli('policy',self.names[2],'submissions_minute','1')
  body={'version':self.version,'source':'def between(value,left,right): return True','language':'python','mode':'submit'};key=str(uuid.uuid4())
  status,data,_=self.bob.request('/submissions','POST',body,{'Idempotency-Key':key});self.assertEqual(status,202)
  self.assertEqual(self.bob.request('/submissions','POST',body,{'Idempotency-Key':key})[1],data)
  self.assertEqual(self.alice.request('/submissions/'+data['id'])[0],404)
  status,data,headers=self.bob.request('/submissions','POST',body,{'Idempotency-Key':str(uuid.uuid4())});self.assertEqual(status,429);self.assertGreater(int(headers['retry-after']),0)
  self.assertEqual(self.bob.request('/me/limits')[1]['submissions_minute'],1)
  self.assertEqual(self.alice.request('/me/limits')[1]['submissions_minute'],6)
 def test_055_password_change_invalidates_reset_links(self):
  token=cli('reset',self.names[2]).split('#')[1]
  self.assertEqual(self.bob.request('/me/password','POST',{'current_password':PASSWORD,'new_password':PASSWORD})[0],200)
  self.assertEqual(Client().request('/reset-password','POST',{'token':token,'password':PASSWORD+'attacker'})[0],400)
  self.assertEqual(sql("SELECT count(*) FROM account_tokens WHERE token_hash='"+hashlib.sha256(token.encode()).hexdigest()+"'"),'0')
  self.assertEqual(self.bob.request('/session')[0],401)
  self.bob.login(self.names[2])
 def test_06_hashed_sessions_revocation_and_reset(self):
  clear_redemption_buckets()
  raw=next(c.value for c in self.alice.jar if c.name=='practice_session')
  self.assertEqual(sql("SELECT count(*) FROM sessions WHERE id='"+raw+"'"),'0')
  self.assertEqual(sql("SELECT count(*) FROM sessions WHERE id='"+hashlib.sha256(raw.encode()).hexdigest()+"'"),'1')
  token=cli('reset',self.names[1]).split('#')[1]
  self.assertEqual(Client().request('/reset-password','POST',{'token':token,'password':PASSWORD+'new'})[0],200)
  self.assertEqual(self.alice.request('/session')[0],401)
  cli('suspend',self.names[2]);self.assertEqual(self.bob.request('/session')[0],401)
  self.assertEqual(sql("SELECT count(*) FROM submissions WHERE user_id='"+self.ids[self.names[2]]+"' AND status!='completed'"),'0')
  self.assertGreater(int(sql('SELECT count(*) FROM security_audit')),0)
 def test_07_shared_http_budget_and_recovery(self):
  cli('policy',self.names[0],'api_rate','1')
  replies=[self.admin.request('/capabilities') for _ in range(35)]
  throttled=[r for r in replies if r[0]==429]
  self.assertTrue(throttled);self.assertGreater(int(throttled[0][2]['retry-after']),0)
  self.assertEqual(self.admin.request('/session','DELETE')[0],200)
 def test_08_redemption_hour_blocks_spam(self):
  clear_redemption_buckets()
  for _ in range(4):status,data,headers=Client().request('/register','POST',{'token':'0'*64,'username':'spam','password':PASSWORD})
  self.assertEqual(status,429);self.assertEqual(data['reason'],'redemption_hour')
if __name__=='__main__':unittest.main(verbosity=2)

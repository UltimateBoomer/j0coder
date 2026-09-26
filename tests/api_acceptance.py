#!/usr/bin/env python3
"""Integration checks against a disposable running API; no mock storage or database."""
import http.cookiejar,json,os,pathlib,urllib.request,urllib.error,uuid,unittest
ORIGIN=os.environ.get('TEST_ORIGIN','http://127.0.0.1:18080')
class Client:
 def __init__(self):self.http=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()));self.csrf=''
 def request(self,path,method='GET',body=None,headers=None):
  req=urllib.request.Request(ORIGIN+'/api/v1'+path,method=method,data=json.dumps(body).encode() if body is not None else None,headers={'Origin':ORIGIN,'Content-Type':'application/json','X-CSRF-Token':self.csrf,**(headers or {})})
  try:
   with self.http.open(req) as r:
    data=r.read();return r.status,json.loads(data) if data else None
  except urllib.error.HTTPError as e:
   code=e.code;raw=e.read();e.close()
   try:return code,json.loads(raw)
   except ValueError:return code,raw.decode()
 def login(self,name,password):
  assert self.request('/session','POST',dict(username=name,password=password))[0]==200
  self.csrf=self.request('/session')[1]['csrf']
class Acceptance(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.admin=Client();cls.admin.login('admin','Integration-password-123')
  cls.user=Client();cls.name='test'+uuid.uuid4().hex[:12];cls.password='Integration-user-password'
  assert cls.admin.request('/admin/users','POST',dict(username=cls.name,password=cls.password))[0]==201
  cls.user.login(cls.name,cls.password)
  cls.sample=json.loads(pathlib.Path('samples/1.json').read_text());cls.sample['title']='API integration '+uuid.uuid4().hex[:8]
  status,data=cls.admin.request('/admin/problems','POST',cls.sample);assert status==200
  cls.problem=data['id'];status,data=cls.admin.request('/admin/problems/'+cls.problem+'/publish','POST');assert status==200;cls.version=data['version']
 def test_rejects_legacy_definitions(self):
  for schema in (1,2):
   status,draft=self.admin.request('/admin/problems','POST',{**self.sample,'schema':schema})
   self.assertEqual(status,200)
   self.assertEqual(self.admin.request('/admin/problems/'+draft['id']+'/publish','POST')[0],400)
   self.assertEqual(self.admin.request('/admin/problems/validate','POST',{**self.sample,'schema':schema})[0],400)
  self.assertEqual(self.admin.request('/admin/problems','POST',{**self.sample,'signature':{'method':'between','params':[],'returns':'bool'}})[0],422)
  without_interface={k:v for k,v in self.sample.items() if k!='interface'}
  self.assertEqual(self.admin.request('/admin/problems','POST',without_interface)[0],422)
 def test_auth_and_csrf(self):
  self.assertEqual(Client().request('/problems')[0],401)
  self.assertEqual(self.user.request('/admin/problems')[0],403)
  self.assertEqual(self.admin.request('/admin/users','POST',dict(username='invalid',password='some-password-here'),{'X-CSRF-Token':'wrong'})[0],403)
  self.assertEqual(self.admin.request('/admin/users','POST',dict(username='invalid',password='some-password-here'), {'Origin':'https://evil.example'})[0],403)
 def test_hidden_and_browse(self):
  code,data=self.user.request('/problems/'+self.problem);self.assertEqual(code,200)
  self.assertEqual(len(data['problem']['tests']),2)
  self.assertNotIn('-2147483648',json.dumps(data))
  self.assertIn('def between(',data['starters']['python'])
  self.assertEqual(self.user.request('/problems?difficulty=easy&tag=scalar')[0],200)
 def test_idempotency_ownership_and_validation(self):
  body=dict(version=self.version,source='def between(value, left, right): return min(left, right) <= value <= max(left, right)',language='python',mode='submit');key=str(uuid.uuid4())
  code,a=self.user.request('/submissions','POST',body,{'Idempotency-Key':key});self.assertEqual(code,202)
  code,b=self.user.request('/submissions','POST',body,{'Idempotency-Key':key});self.assertEqual(a,b)
  self.assertEqual(self.user.request('/submissions','POST',{**body,'source':'different'},{'Idempotency-Key':key})[0],409)
  self.assertEqual(self.admin.request('/submissions/'+a['id'])[0],404)
  self.assertEqual(self.user.request('/submissions/'+a['id'])[0],200)
  self.assertEqual(self.user.request('/submissions','POST',{**body,'mode':'run','cases':[dict(args=[True,0,2])]}, {'Idempotency-Key':str(uuid.uuid4())})[0],400)
  self.assertEqual(self.user.request('/submissions','POST',{**body,'mode':'run','cases':[dict(args=[2147483648,0,2])]}, {'Idempotency-Key':str(uuid.uuid4())})[0],400)
 def test_publish_immutable(self):
  old=self.user.request('/problems/'+self.problem)[1]
  solution='/solutions/'+self.version+'/python'
  self.assertEqual(self.user.request(solution,'PUT',{'source':'saved before republishing'})[0],204)
  draft={**self.sample,'title':'Updated immutable title'}
  self.assertEqual(self.admin.request('/admin/problems/'+self.problem,'PUT',draft)[0],204)
  self.assertEqual(self.user.request('/problems/'+self.problem)[1],old)
  status,new=self.admin.request('/admin/problems/'+self.problem+'/publish','POST');self.assertEqual(status,200);self.assertNotEqual(new['version'],self.version)
  self.assertEqual(self.user.request('/solutions/'+new['version']+'/python')[0],404)
  self.assertEqual(self.user.request(solution)[1]['source'],'saved before republishing')
  bad={**draft,'signature':{'method':'between','params':[],'returns':'bool'}}
  self.assertEqual(self.admin.request('/admin/problems/'+self.problem,'PUT',bad)[0],422)
  self.assertEqual(self.admin.request('/admin/problems/'+self.problem+'/publish','POST')[0],200)
 def test_solution_drafts(self):
  subject=Client();name='draft'+uuid.uuid4().hex[:12];password='Integration-draft-password'
  self.assertEqual(self.admin.request('/admin/users','POST',dict(username=name,password=password))[0],201)
  subject.login(name,password)
  path='/solutions/'+self.version+'/cpp'
  self.assertEqual(Client().request(path)[0],401)
  self.assertEqual(subject.request(path)[0],404)
  self.assertEqual(subject.request(path,'PUT',{'source':'first'})[0],204)
  status,draft=subject.request(path);self.assertEqual(status,200);self.assertEqual(draft['source'],'first');self.assertIn('updated_at',draft)
  self.assertEqual(subject.request(path,'PUT',{'source':'second'})[0],204)
  self.assertEqual(subject.request(path)[1]['source'],'second')
  self.assertEqual(self.user.request(path)[0],404)
  self.assertEqual(self.admin.request(path)[0],404)
  self.assertEqual(self.admin.request(path,'PUT',{'source':'admin code'})[0],204)
  self.assertEqual(subject.request(path)[1]['source'],'second')
  self.assertEqual(subject.request('/solutions/'+self.version+'/python')[0],404)
  self.assertEqual(subject.request('/solutions/'+self.version+'/invalid','PUT',{'source':'x'})[0],400)
  self.assertEqual(subject.request('/solutions/'+self.version+'/invalid')[0],400)
  self.assertEqual(subject.request(path,'PUT',{'source':'é'*50001})[0],400)
  self.assertEqual(subject.request(path,'PUT',{'source':'x'},{'X-CSRF-Token':'wrong'})[0],403)
  self.assertEqual(subject.request(path,'PUT',{'source':'x'},{'Origin':'https://evil.example'})[0],403)
  self.assertEqual(subject.request('/solutions/'+str(uuid.uuid4())+'/cpp','PUT',{'source':'x'})[0],404)
  self.assertEqual(subject.request(path)[1]['source'],'second')
if __name__=='__main__':unittest.main(verbosity=2)

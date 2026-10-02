#!/usr/bin/env python3
"""Versioned REST schema. Regenerate checked-in TypeScript with npm run generate:api."""
import json
import subprocess
ref=lambda n:{'$ref':'#/components/schemas/'+n}
obj=lambda p,required=None:{'type':'object','properties':p,'required':required or list(p)}
string={'type':'string'};uuid={'type':'string','format':'uuid'};boolean={'type':'boolean'}
array=lambda x:{'type':'array','items':x}
schemas={
'User':obj({'id':uuid,'username':string,'admin':boolean,'csrf':string}),
'Credentials':obj({'username':string,'password':{'type':'string','maxLength':256}}),
'Verdict':{'type':'string','enum':['accepted','wrong_answer','compilation_error','runtime_error','time_limit','memory_limit','output_limit','infrastructure_failure','cancelled']},
'Outcome':obj({'elapsed_ms':{'type':'integer'},'verdict':ref('Verdict'),'passed':{'type':'integer'},'total':{'type':'integer'},'cases':array(obj({'hidden':boolean,'verdict':{'anyOf':[ref('Verdict'),{'type':'null'}]},'output':{},'log':string})),'diagnostic':{'type':['string','null']} }),
'Submission':obj({'id':uuid,'version':uuid,'status':{'type':'string','enum':['queued','running','completed']},'result':{'anyOf':[ref('Outcome'),{'type':'null'}]}}),
'Submit':obj({'version':uuid,'language':ref('Language'),'source':{'type':'string','maxLength':100000},'mode':{'type':'string','enum':['run','submit']},'cases':array(ref('CaseInput'))},['version','language','source','mode']),
'SolutionDraft':obj({'source':string,'updated_at':{'type':'string','format':'date-time'}}),
'Error':obj({'error':string,'reason':string},['error'])}
schemas.update(json.loads(subprocess.check_output(["cargo", "run", "--locked", "--quiet", "--bin", "schema-export", "--", "api-components"])))
paths={}
def endpoint(path,method,name,response,body=None,code='200',params=None):
 op={'operationId':name,'responses':{code:{'description':'Success','content':{'application/json':{'schema':response}}},**{str(c):{'description':d,'content':{'application/json':{'schema':ref('Error')}}}for c,d in [(400,'Invalid request'),(401,'Login required'),(403,'Forbidden'),(409,'Idempotency conflict'),(429,'Capacity exceeded'),(500,'Service unavailable')]}},'security':[{'session':[]}]}
 op['parameters']=params or []
 if '{id}' in path:op['parameters'].append({'name':'id','in':'path','required':True,'schema':uuid})
 if method!='get':op['parameters'] += [{'name':'Origin','in':'header','required':True,'schema':string},{'name':'X-CSRF-Token','in':'header','required':True,'schema':string}]
 if body:op['requestBody']={'required':True,'content':{'application/json':{'schema':body}}}
 paths['/api/v1'+path]=paths.get('/api/v1'+path,{})|{method:op}
endpoint('/session','get','getSession',ref('User'))
endpoint('/session','post','login',obj({'ok':boolean}),ref('Credentials'))
endpoint('/session','delete','logout',obj({'ok':boolean}))
endpoint('/problems','get','listProblems',array(ref('ProblemSummary')),params=[{'name':n,'in':'query','schema':string} for n in ['q','tag','difficulty','cursor']]+[{'name':n,'in':'query','schema':{'type':'integer'}} for n in ['min_score','max_score','limit']])
endpoint('/problems/{id}','get','getProblem',ref('ProblemDetail'))
solution_params=[{'name':'version','in':'path','required':True,'schema':uuid},{'name':'language','in':'path','required':True,'schema':ref('Language')}]
endpoint('/solutions/{version}/{language}','get','getSolution',ref('SolutionDraft'),params=solution_params.copy())
endpoint('/solutions/{version}/{language}','put','putSolution',{},obj({'source':{'type':'string','description':'At most 100000 UTF-8 bytes'}}),'204',solution_params.copy())

endpoint('/admin/problems','get','listDrafts',array(ref('CatalogDraft')))
endpoint('/admin/problems','post','createDraft',ref('CreatedProblem'),ref('ProblemInput'))
endpoint('/admin/problems/validate','post','validateProblem',ref('ValidatedDefinition'),ref('ProblemInput'))
endpoint('/admin/catalog','get','getCatalogStatus',{})

endpoint('/admin/problems/{id}','put','saveDraft',{},ref('ProblemInput'),'204')
endpoint('/admin/problems/{id}/publish','post','publish',ref('PublishedProblem'))
endpoint('/submissions','post','submit',obj({'id':uuid}),ref('Submit'),'202',[{'name':'Idempotency-Key','in':'header','required':True,'schema':string}])
endpoint('/submissions','get','history',array({}),params=[{'name':'version','in':'query','required':True,'schema':uuid}])
endpoint('/submissions/{id}','get','getSubmission',ref('Submission'))
endpoint('/editor-ticket','post','editorTicket',obj({'ticket':string,'path':string}),obj({'language':ref('Language')}))
prefs={'theme':{'enum':['system','light','dark']},'default_language':ref('Language'),'semantic_completion':boolean,'font_size':{'type':'integer','minimum':10,'maximum':24},'tab_width':{'enum':[2,4,8]},'word_wrap':boolean,'minimap':boolean,'blind_mode':boolean}
schemas['Preferences']=obj(prefs)
schemas['Preferences']['additionalProperties']=False
endpoint('/capabilities','get','capabilities',ref('Capabilities'))
endpoint('/register','post','register',{},obj({'token':string,'username':string,'password':{'type':'string','minLength':15,'maxLength':256}}),'201')
endpoint('/reset-password','post','resetPassword',obj({'ok':boolean}),obj({'token':string,'password':{'type':'string','minLength':15,'maxLength':256}}))
endpoint('/me/preferences','get','getPreferences',ref('Preferences'))
endpoint('/me/preferences','patch','updatePreferences',ref('Preferences'),dict(obj(prefs,[]),required=[],additionalProperties=False))
endpoint('/me/limits','get','getLimits',obj({k:{'type':'integer'}for k in ['api_rate','save_rate','submissions_minute','submissions_day','pending','ticket_rate','editor_sessions','editor_messages','editor_bytes']}))
endpoint('/me/password','post','changePassword',obj({'ok':boolean}),obj({'current_password':string,'new_password':{'type':'string','minLength':15,'maxLength':256}}))
endpoint('/me/logout-all','post','logoutAll',obj({'ok':boolean}))
endpoint('/me/reauthenticate','post','reauthenticate',obj({'ok':boolean}),ref('Credentials'))
for path in ['/session','/register','/reset-password','/capabilities']:
 for method,op in paths['/api/v1'+path].items():
  if method=='post' or path=='/capabilities':
   op['security']=[]
   op['parameters']=[p for p in op['parameters'] if p['name']!='X-CSRF-Token']
for path in ['/problems','/problems/{id}']:
 paths['/api/v1'+path]['get']['security']=[{'session':[]},{}]
 paths['/api/v1'+path]['get']['description']='Anonymous access only when guest browsing is enabled.'
for path,item in paths.items():
 for op in item.values():
  op['responses']['503']={'description':'Admission unavailable','content':{'application/json':{'schema':ref('Error')}}}
  op['responses']['429']['headers']={'Retry-After':{'schema':{'type':'integer'},'description':'Seconds before retry'}}
  if '/admin/' in path:
   op['description']='Private admin listener only; absent from public listener.'
   op['security']=[{'adminSession':[]}]

open('openapi.json','w').write(json.dumps({'openapi':'3.1.0','info':{'title':'j0coder API','version':'1.0.0'},'paths':paths,'components':{'schemas':schemas,'securitySchemes':{'session':{'type':'apiKey','in':'cookie','name':'practice_session'},'adminSession':{'type':'apiKey','in':'cookie','name':'practice_admin'}}}},indent=2)+'\n')

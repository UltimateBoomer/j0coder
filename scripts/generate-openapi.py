#!/usr/bin/env python3
"""Versioned REST schema. Regenerate checked-in TypeScript with npm run generate:api."""
import json
ref=lambda n:{'$ref':'#/components/schemas/'+n}
obj=lambda p,required=None:{'type':'object','properties':p,'required':required or list(p)}
string={'type':'string'};uuid={'type':'string','format':'uuid'};boolean={'type':'boolean'}
array=lambda x:{'type':'array','items':x}
schemas={
'Type':{'oneOf':[{'type':'string','enum':['int','bool','string']},obj({'array':ref('Type')})]},
'Parameter':obj({'name':string,'ty':ref('Type')}),
'Signature':obj({'method':string,'params':array(ref('Parameter')),'returns':ref('Type')}),
'Limits':obj({'time_ms':{'type':'integer'},'memory_mib':{'type':'integer'},'output_bytes':{'type':'integer'}}),
'Case':obj({'args':array({}),'expected':{},'hidden':boolean},['args']),
'Problem':obj({'title':string,'statement':string,'difficulty':{'type':'string','enum':['easy','medium','hard']},'tags':array(string),'signature':ref('Signature'),'limits':ref('Limits'),'tests':array(ref('Case'))}),
'User':obj({'id':uuid,'username':string,'admin':boolean,'csrf':string}),
'Credentials':obj({'username':string,'password':{'type':'string','minLength':12,'maxLength':256}}),
'Language':{'type':'string','enum':['cpp','python']},
'Verdict':{'type':'string','enum':['accepted','wrong_answer','compilation_error','runtime_error','time_limit','memory_limit','output_limit','infrastructure_failure']},
'Outcome':obj({'elapsed_ms':{'type':'integer'},'verdict':ref('Verdict'),'passed':{'type':'integer'},'total':{'type':'integer'},'cases':array(obj({'hidden':boolean,'verdict':{'anyOf':[ref('Verdict'),{'type':'null'}]},'output':{},'log':string})),'diagnostic':{'type':['string','null']} }),
'Submission':obj({'id':uuid,'version':uuid,'status':{'type':'string','enum':['queued','running','completed']},'result':{'anyOf':[ref('Outcome'),{'type':'null'}]}}),
'Submit':obj({'version':uuid,'language':ref('Language'),'source':{'type':'string','maxLength':100000},'mode':{'type':'string','enum':['run','submit']},'cases':array(ref('Case'))},['version','language','source','mode']),
'ProblemDetail':obj({'id':uuid,'version':uuid,'problem':ref('Problem'),'starters':obj({'cpp':string,'python':string})}),
'ProblemSummary':obj({'id':uuid,'version':uuid,'title':string,'difficulty':string,'tags':array(string)}),
'Error':obj({'error':string})}
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
endpoint('/problems','get','listProblems',array(ref('ProblemSummary')),params=[{'name':n,'in':'query','schema':string} for n in ['q','tag','difficulty']])
endpoint('/problems/{id}','get','getProblem',ref('ProblemDetail'))
endpoint('/admin/users','post','createUser',{},ref('Credentials'),'201')
endpoint('/admin/problems','get','listDrafts',array(obj({'id':uuid,'draft':ref('Problem'),'version':{'type':['string','null']}})))
endpoint('/admin/problems','post','createDraft',obj({'id':uuid}),ref('Problem'))
endpoint('/admin/problems/{id}','put','saveDraft',{},ref('Problem'),'204')
endpoint('/admin/problems/{id}/publish','post','publish',obj({'version':uuid}))
endpoint('/submissions','post','submit',obj({'id':uuid}),ref('Submit'),'202',[{'name':'Idempotency-Key','in':'header','required':True,'schema':string}])
endpoint('/submissions','get','history',array({}),params=[{'name':'version','in':'query','required':True,'schema':uuid}])
endpoint('/submissions/{id}','get','getSubmission',ref('Submission'))
endpoint('/editor-ticket','post','editorTicket',obj({'ticket':string,'path':string}),obj({'language':ref('Language')}))
open('openapi.json','w').write(json.dumps({'openapi':'3.1.0','info':{'title':'Practice API','version':'1.0.0'},'paths':paths,'components':{'schemas':schemas,'securitySchemes':{'session':{'type':'apiKey','in':'cookie','name':'practice_session'}}}},indent=2)+'\n')

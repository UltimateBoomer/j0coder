#!/usr/bin/env python3
"""Versioned REST schema. Regenerate checked-in TypeScript with npm run generate:api."""
import json
ref=lambda n:{'$ref':'#/components/schemas/'+n}
obj=lambda p,required=None:{'type':'object','properties':p,'required':required or list(p)}
string={'type':'string'};uuid={'type':'string','format':'uuid'};boolean={'type':'boolean'}
array=lambda x:{'type':'array','items':x}
schemas={
'Type':{'oneOf':[{'type':'string','enum':['void','int','int64','float','bool','string']},obj({'array':ref('Type')}),obj({'nullable':ref('Type')}),obj({'named':string})]},
'Parameter':obj({'name':string,'ty':ref('Type'),'constraints':{'type':'string','description':'Optional display-only Markdown; at most 1000 UTF-8 bytes'}},['name','ty']),
'Field':obj({'name':string,'ty':ref('Type')}),
'TypeDefinition':obj({'name':string,'codec':{'type':'string','enum':['record','singly_linked_list','binary_tree','nary_tree','object_graph']},'fields':array(ref('Field'))},['name']),
'Comparison':obj({'array':{'type':'string','enum':['ordered','set','multiset']},'absolute_tolerance':{'type':'number'},'relative_tolerance':{'type':'number'}},[]),
'Limits':obj({'time_ms':{'type':'integer'},'memory_mib':{'type':'integer'},'output_bytes':{'type':'integer'}}),
'FunctionInterface':obj({'kind':{'const':'function'},'name':string,'params':array(ref('Parameter')),'returns':ref('Type')}),
'Constructor':obj({'params':array(ref('Parameter'))}),
'Method':obj({'name':string,'params':array(ref('Parameter')),'returns':ref('Type')}),
'DataStructureInterface':obj({'kind':{'const':'data_structure'},'name':string,'constructor':ref('Constructor'),'methods':array(ref('Method'))}),
'Interface':{'oneOf':[ref('FunctionInterface'),ref('DataStructureInterface')]},
'FunctionCase':obj({'args':array({}),'expected':{},'hidden':boolean},['args','expected']),
'Operation':obj({'method':string,'args':array({}),'expected':{}},['method','args','expected']),
'StatefulCase':obj({'constructor_args':array({}),'operations':array(ref('Operation')),'hidden':boolean},['constructor_args','operations']),
'Case':{'oneOf':[ref('FunctionCase'),ref('StatefulCase')]},
'Problem':obj({'schema':{'type':'integer','enum':[3]},'title':string,'statement':string,'hints':array(string),'difficulty':{'type':'string','enum':['easy','medium','hard']},'tags':array(string),'interface':ref('Interface'),'type_definitions':array(ref('TypeDefinition')),'comparison':ref('Comparison'),'limits':ref('Limits'),'tests':array(ref('Case'))},['schema','title','statement','difficulty','tags','interface','limits','tests']),
'User':obj({'id':uuid,'username':string,'admin':boolean,'csrf':string}),
'Credentials':obj({'username':string,'password':{'type':'string','minLength':12,'maxLength':256}}),
'Language':{'type':'string','enum':['cpp','python','java','kotlin']},
'Verdict':{'type':'string','enum':['accepted','wrong_answer','compilation_error','runtime_error','time_limit','memory_limit','output_limit','infrastructure_failure']},
'Outcome':obj({'elapsed_ms':{'type':'integer'},'verdict':ref('Verdict'),'passed':{'type':'integer'},'total':{'type':'integer'},'cases':array(obj({'hidden':boolean,'verdict':{'anyOf':[ref('Verdict'),{'type':'null'}]},'output':{},'log':string})),'diagnostic':{'type':['string','null']} }),
'Submission':obj({'id':uuid,'version':uuid,'status':{'type':'string','enum':['queued','running','completed']},'result':{'anyOf':[ref('Outcome'),{'type':'null'}]}}),
'Submit':obj({'version':uuid,'language':ref('Language'),'source':{'type':'string','maxLength':100000},'mode':{'type':'string','enum':['run','submit']},'cases':array(ref('Case'))},['version','language','source','mode']),
'ProblemDetail':obj({'id':uuid,'version':uuid,'problem':ref('Problem'),'starters':obj({'cpp':string,'python':string,'java':string,'kotlin':string})}),
'ProblemSummary':obj({'id':uuid,'version':uuid,'title':string,'summary':string,'difficulty':string,'difficulty_score':{'type':'integer','minimum':1,'maximum':5},'tags':array(string)}),
'SolutionDraft':obj({'source':string,'updated_at':{'type':'string','format':'date-time'}}),
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
endpoint('/problems','get','listProblems',array(ref('ProblemSummary')),params=[{'name':n,'in':'query','schema':string} for n in ['q','tag','difficulty','cursor']]+[{'name':n,'in':'query','schema':{'type':'integer'}} for n in ['min_score','max_score','limit']])
endpoint('/problems/{id}','get','getProblem',ref('ProblemDetail'))
solution_params=[{'name':'version','in':'path','required':True,'schema':uuid},{'name':'language','in':'path','required':True,'schema':ref('Language')}]
endpoint('/solutions/{version}/{language}','get','getSolution',ref('SolutionDraft'),params=solution_params.copy())
endpoint('/solutions/{version}/{language}','put','putSolution',{},obj({'source':{'type':'string','description':'At most 100000 UTF-8 bytes'}}),'204',solution_params.copy())
endpoint('/admin/users','post','createUser',{},ref('Credentials'),'201')
endpoint('/admin/problems','get','listDrafts',array(obj({'id':uuid,'draft':ref('Problem'),'version':{'type':['string','null']},'managed':boolean})))
endpoint('/admin/problems','post','createDraft',obj({'id':uuid}),ref('Problem'))
endpoint('/admin/problems/validate','post','validateProblem',obj({'valid':boolean,'content_hash':string}),ref('Problem'))
endpoint('/admin/catalog','get','getCatalogStatus',{})
endpoint('/admin/catalog','put','updateCatalogSettings',{},obj({'repository_url':string,'strategy':{'type':'string','enum':['track_branch','pinned_commit']},'revision':string,'poll_interval_seconds':{'type':'integer','minimum':10,'maximum':86400},'enabled':boolean,'generation':{'type':'integer'}}))
endpoint('/admin/problems/{id}','put','saveDraft',{},ref('Problem'),'204')
endpoint('/admin/problems/{id}/publish','post','publish',obj({'version':uuid}))
endpoint('/submissions','post','submit',obj({'id':uuid}),ref('Submit'),'202',[{'name':'Idempotency-Key','in':'header','required':True,'schema':string}])
endpoint('/submissions','get','history',array({}),params=[{'name':'version','in':'query','required':True,'schema':uuid}])
endpoint('/submissions/{id}','get','getSubmission',ref('Submission'))
endpoint('/editor-ticket','post','editorTicket',obj({'ticket':string,'path':string}),obj({'language':ref('Language')}))
open('openapi.json','w').write(json.dumps({'openapi':'3.1.0','info':{'title':'Locoder API','version':'1.0.0'},'paths':paths,'components':{'schemas':schemas,'securitySchemes':{'session':{'type':'apiKey','in':'cookie','name':'practice_session'}}}},indent=2)+'\n')

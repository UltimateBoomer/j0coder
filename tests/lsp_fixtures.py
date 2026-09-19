"""Trusted packaged-language-server smoke test, not an isolation acceptance test."""
import json,os,subprocess,threading,queue,time

def check(command,language,filename,source,line,character):
 p=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,cwd='/workspace')
 messages=queue.Queue()
 diagnostics=[]
 def read():
  while True:
   length=None
   while True:
    header=p.stdout.readline()
    if not header:return
    if header==b'\r\n':break
    if header.lower().startswith(b'content-length:'):length=int(header.split(b':')[1])
   messages.put(json.loads(p.stdout.read(length)))
 threading.Thread(target=read,daemon=True).start()
 def send(v):
  data=json.dumps({'jsonrpc':'2.0',**v}).encode();p.stdin.write(f'Content-Length: {len(data)}\r\n\r\n'.encode()+data);p.stdin.flush()
 def wait(id):
  deadline=time.monotonic()+20
  while time.monotonic()<deadline:
   v=messages.get(timeout=20)
   if v.get('method')=='textDocument/publishDiagnostics':diagnostics.extend(v['params']['diagnostics'])
   if v.get('id')==id and 'method' not in v:return v
   if 'id' in v and 'method' in v:send({'id':v['id'],'result':[{}] if v['method']=='workspace/configuration' else None})
  raise AssertionError('timeout')
 try:
  send({'id':1,'method':'initialize','params':{'processId':None,'rootUri':'file:///workspace','capabilities':{'textDocument':{'completion':{'completionItem':{'snippetSupport':True}}}}}})
  init=wait(1);assert 'result' in init,init
  send({'method':'initialized','params':{}})
  uri='file:///workspace/'+filename
  send({'method':'textDocument/didOpen','params':{'textDocument':{'uri':uri,'languageId':language,'version':1,'text':source}}})
  time.sleep(1)
  send({'id':2,'method':'textDocument/completion','params':{'textDocument':{'uri':uri},'position':{'line':line,'character':character}}})
  completion=wait(2);result=completion.get('result');items=result.get('items',[]) if isinstance(result,dict) else result
  assert items,completion
  assert diagnostics,(language,'expected diagnostics for incomplete expression')
  print(language,'semantic completion:',len(items),'items;',len(diagnostics),'diagnostics')
 finally:p.kill();p.wait()
check(['clangd','--background-index=false','--compile-commands-dir=/workspace'],'cpp','solution.cpp','#include <vector>\nint main(){ std::vector<int> values; values. }\n',1,42)
check(['pyright-langserver','--stdio'],'python','solution.py','value = "hello"\nvalue.\n',1,6)

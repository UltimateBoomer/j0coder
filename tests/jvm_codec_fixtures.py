"""Trusted JVM codec fixtures over the gVisor streaming protocol.

Run with TOOLCHAIN_IMAGE and SANDBOX_RUNTIME set to the pinned image and runsc path.
"""
import base64,json,os,subprocess
IMAGE=os.environ['TOOLCHAIN_IMAGE']
RUNTIME=os.environ['SANDBOX_RUNTIME']
assert RUNTIME.rsplit('/',1)[-1]=='runsc', 'gVisor runsc is required'

def harness(mode,name,content,case='',memory=256):
    request=json.dumps({'name':name,'data':base64.b64encode(content).decode(),'input':case}).encode()
    cmd=['podman','run','--rm','-i','--runtime',RUNTIME,'--cpus=1','--network=none','--security-opt=label=disable','--cap-drop=ALL','--security-opt=no-new-privileges','--read-only','--pids-limit=64','--memory','2048m' if name=='solution.kt' else '1024m' if mode=='compile' else '512m','--tmpfs','/input:rw,size=48m,mode=1777','--tmpfs','/work:rw,exec,size=128m,mode=1777','-e','LOCODER_STREAM_PROTOCOL=1',IMAGE,'python3','/opt/harness.py',mode,'1048576',str(memory)]
    p=subprocess.run(cmd,input=request,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=150)
    if p.returncode:raise RuntimeError(p.stderr.decode())
    try:r=json.loads(p.stdout)
    except Exception:raise RuntimeError(p.stdout.decode()+p.stderr.decode())
    return r

def named(x):return {'named':x}
def nullable(x):return {'nullable':x}
def array(x):return {'array':x}
def field(name,ty):return {'name':name,'ty':ty}
def defn(name,codec,fields):return {'name':name,'codec':codec,'fields':fields}
def param(name,ty):return {'name':name,'ty':ty}
D=[
 defn('Box','record',[field('value','int64'),field('label',nullable('string')),field('items',array(nullable('int')))]),
 defn('Link','singly_linked_list',[field('val','int'),field('next',nullable(named('Link')))]),
 defn('Tree','binary_tree',[field('val','int'),field('left',nullable(named('Tree'))),field('right',nullable(named('Tree')))]),
 defn('Nary','nary_tree',[field('val','int'),field('children',array(named('Nary')))]),
 defn('Graph','object_graph',[field('value','int'),field('next',nullable(named('Graph')))]),
]
methods=[{'name':n,'params':[param('x',named(t))],'returns':named(t)} for n,t in [('record','Box'),('list','Link'),('tree','Tree'),('nary','Nary'),('graph','Graph')]]
iface={'kind':'data_structure','name':'Store','constructor':{'params':[]},'methods':methods}
java='''import java.util.*;
class Box { public Long value; public String label; public List<Integer> items; public Box() {} }
class Link { public Integer val; public Link next; public Link() {} }
class Tree { public Integer val; public Tree left; public Tree right; public Tree() {} }
class Nary { public Integer val; public List<Nary> children; public Nary() {} }
class GraphNode { public String __judgeId; public Integer value; public GraphNode next; public GraphNode() {} }
class Graph { public List<GraphNode> roots = new ArrayList<>(); public List<GraphNode> nodes = new ArrayList<>(); }
class Store { public Store() {} public Box record(Box x){return x;} public Link list(Link x){return x;} public Tree tree(Tree x){return x;} public Nary nary(Nary x){return x;} public Graph graph(Graph x){GraphNode n=new GraphNode();n.value=3;n.next=x.roots.get(0);x.roots.add(n);return x;} }
'''
kotlin='''class Box { var value: Long = 0; var label: String? = null; lateinit var items: MutableList<Int?> }
class Link { var `val`: Int = 0; var next: Link? = null }
class Tree { var `val`: Int = 0; var left: Tree? = null; var right: Tree? = null }
class Nary { var `val`: Int = 0; lateinit var children: MutableList<Nary> }
class GraphNode { var __judgeId: String? = null; var value: Int = 0; var next: GraphNode? = null }
class Graph { var roots: MutableList<GraphNode?> = mutableListOf(); var nodes: MutableList<GraphNode> = mutableListOf() }
class Store { fun record(x: Box) = x; fun list(x: Link?) = x; fun tree(x: Tree?) = x; fun nary(x: Nary?) = x; fun graph(x: Graph): Graph { val n=GraphNode();n.value=3;n.next=x.roots[0];x.roots.add(n);return x } }
'''
graph={'roots':['a','a'],'nodes':[{'id':'a','value':1,'next':'b'},{'id':'b','value':2,'next':'a'}]}
values=[{'value':9223372036854775807,'label':None,'items':[1,None,-1]},[1,2,3],[1,None,2,3],{'value':1,'children':[{'value':2,'children':[]}]},graph]
expected=values[:-1]+[{'roots':['a','a','new1'],'nodes':[{'id':'a','value':1,'next':'b'},{'id':'b','value':2,'next':'a'},{'id':'new1','value':3,'next':'a'}]}]
case={'constructor_args':[],'operations':[{'method':m['name'],'args':[v]} for m,v in zip(methods,values)]}
case['operations'].append({'method':'nary','args':[None]})
expected.append(None)
for language,source,name in [('java',java,'solution.java'),('kotlin',kotlin,'solution.kt')]:
 schema={'source':source,'interface':iface,'type_definitions':D,'language':language}
 c=harness('compile',name,json.dumps(schema).encode())
 print(language,'compile',c['exit'],c['log'][:700])
 assert c['exit']==0 and c['artifact']
 r=harness('run','program.jar.b64',c['artifact'].encode(),json.dumps(case))
 print(language,'run',r['exit'],str(r['output'])[:700],r['log'][:700])
 assert r['exit']==0 and r['output']==expected

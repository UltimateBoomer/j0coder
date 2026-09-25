#!/usr/bin/env python3
"""Independent fixture oracle for authored catalog batches 037-071 and 111-120."""
import json, os, statistics, collections, heapq, itertools, glob, sys
from pathlib import Path
P=str(Path(sys.argv[1] if len(sys.argv)>1 else '../code-practice-problems')/'problems')
S='balanced-delimiters postfix-expression minimum-stack next-warmer-day maximum-in-every-window largest-histogram-rectangle first-position-at-least-target find-in-rotated-order minimum-in-rotated-order median-of-two-sorted-sequences smallest-feasible-capacity kth-pair-distance tree-height mirror-binary-tree values-by-level valid-search-tree lowest-shared-ancestor kth-search-tree-value build-tree-from-traversals longest-tree-path height-balanced-tree maximum-tree-path-sum reverse-a-list merge-sorted-lists remove-nth-node-from-the-end reorder-list-ends add-list-encoded-integers merge-k-sorted-lists most-frequent-k-values k-largest-values running-median minimum-concurrent-rooms insert-an-interval fewest-arrows-for-intervals merge-k-sorted-arrays largest-all-one-rectangle all-valid-word-breaks place-the-fewest-tree-cameras repair-a-swapped-search-tree kth-value-in-a-sorted-matrix infer-an-alien-alphabet critical-network-connections cheapest-route-with-stop-limit sliding-window-median three-nonoverlapping-maximum-sum-windows'.split()
def parse(vals):
 if not vals:return None
 nodes=[[vals[0],None,None]];q=[0];k=1
 while q and k<len(vals):
  i=q.pop(0)
  for side in (1,2):
   if k<len(vals):
    v=vals[k];k+=1
    if v is not None:
     j=len(nodes);nodes.append([v,None,None]);nodes[i][side]=j;q.append(j)
 return (nodes,0)
def serial(t):
 if t is None:return []
 ns,r=t;out=[];q=[r]
 while q:
  i=q.pop(0)
  if i is None:out.append(None)
  else:
   v,l,rr=ns[i];out.append(v);q.extend([l,rr])
 while out and out[-1] is None:out.pop()
 return out
def solve(slug,a):
 x=a[0]
 if slug=='balanced-delimiters':
  st=[];m={')':'(',']':'[','}':'{'}
  for ch in x:
   if ch in '([{':st.append(ch)
   elif ch in m:
    if not st or st.pop()!=m[ch]:return False
  return not st
 if slug=='postfix-expression':
  s=[]
  for t in x:
   if t not in '+-*/':s.append(int(t));continue
   b=s.pop();q=s.pop();s.append(q+b if t=='+' else q-b if t=='-' else q*b if t=='*' else (abs(q)//abs(b))*(-1 if (q<0)^(b<0) else 1))
  return s[0]
 if slug=='next-warmer-day':return [next((j-i for j in range(i+1,len(x)) if x[j]>v),0) for i,v in enumerate(x)]
 if slug=='maximum-in-every-window':return [max(x[i:i+a[1]]) for i in range(len(x)-a[1]+1)]
 if slug=='largest-histogram-rectangle':return max([min(x[i:j+1])*(j-i+1) for i in range(len(x)) for j in range(i,len(x))]+[0])
 if slug=='first-position-at-least-target':return next((i for i,v in enumerate(x) if v>=a[1]),len(x))
 if slug=='find-in-rotated-order':return x.index(a[1]) if a[1] in x else -1
 if slug=='minimum-in-rotated-order':return min(x)
 if slug=='median-of-two-sorted-sequences':return statistics.median(sorted(x+a[1]))
 if slug=='smallest-feasible-capacity':
  lo=max(x);hi=sum(x)
  while lo<hi:
   m=(lo+hi)//2;days=1;load=0
   for v in x:
    if load+v>m:days+=1;load=0
    load+=v
   if days<=a[1]:hi=m
   else:lo=m+1
  return lo
 if slug=='kth-pair-distance':return sorted(abs(u-v) for i,u in enumerate(x) for v in x[i+1:])[a[1]-1]
 if slug in ('tree-height','mirror-binary-tree','values-by-level','valid-search-tree','lowest-shared-ancestor','kth-search-tree-value','build-tree-from-traversals','longest-tree-path','height-balanced-tree','maximum-tree-path-sum','place-the-fewest-tree-cameras','repair-a-swapped-search-tree'):
  if slug=='build-tree-from-traversals':
   pre,ino=x,a[1]
   def build(p,seq):
    if not seq:return None
    v=p[0];i=seq.index(v);return [v,build(p[1:i+1],seq[:i]),build(p[i+1:],seq[i+1:])]
   def flat(t):
    if t is None:return []
    out=[];q=[t]
    while q:
     z=q.pop(0);out.append(None if z is None else z[0])
     if z:q+=z[1:]
    while out and out[-1] is None:out.pop()
    return out
   return flat(build(pre,ino))
  t=parse(x)
  if slug=='tree-height':
   def f(i):return 0 if i is None else 1+max(f(t[0][i][1]),f(t[0][i][2]))
   return f(t[1]) if t else 0
  if slug=='mirror-binary-tree':
   if not t:return []
   ns,r=t
   def f(i):
    if i is None:return None
    v,l,rr=ns[i];return [v,f(rr),f(l)]
   return flat_tree(f(r))
  if slug=='values-by-level':
   if not t:return []
   ns,r=t;out=[];q=[r]
   while q:
    lev=[]
    for _ in range(len(q)):
     i=q.pop(0);v,l,rr=ns[i];lev.append(v)
     if l is not None:q.append(l)
     if rr is not None:q.append(rr)
    out.append(lev)
   return out
  if slug=='valid-search-tree':
   if not t:return True
   ns,r=t
   def f(i,lo,hi):
    if i is None:return True
    v,l,rr=ns[i];return (lo is None or lo<v) and (hi is None or v<hi) and f(l,lo,v) and f(rr,v,hi)
   return f(r,None,None)
  if slug=='lowest-shared-ancestor':
   ns,r=t;paths={}
   def f(i,path):
    v,l,rr=ns[i];paths[v]=path+[i]
    if l is not None:f(l,path+[i])
    if rr is not None:f(rr,path+[i])
   f(r,[]);u=paths[a[1]];v=paths[a[2]];ans=r
   for q,w in zip(u,v):
    if q!=w:break
    ans=q
   return ns[ans][0]
  if slug=='kth-search-tree-value':
   ns,r=t;v=[]
   def f(i):
    if i is not None:
     z,l,rr=ns[i];f(l);v.append(z);f(rr)
   f(r);return v[a[1]-1]
  if slug=='longest-tree-path':
   if not t:return 0
   best=0
   def f(i):
    nonlocal best
    if i is None:return 0
    _,l,r=ns[i];u=f(l);v=f(r);best=max(best,u+v);return max(u,v)+1
   ns,r=t;f(r);return best
  if slug=='height-balanced-tree':
   if not t:return True
   ok=True
   def f(i):
    nonlocal ok
    if i is None:return 0
    _,l,r=ns[i];u=f(l);v=f(r);ok &= abs(u-v)<=1;return max(u,v)+1
   ns,r=t;f(r);return bool(ok)
  if slug=='maximum-tree-path-sum':
   best=-10**30
   def f(i):
    nonlocal best
    if i is None:return 0
    v,l,r=ns[i];u=max(0,f(l));w=max(0,f(r));best=max(best,v+u+w);return v+max(u,w)
   ns,r=t;f(r);return best
  if slug=='place-the-fewest-tree-cameras':
   if not t:return 0
   ns,r=t;n=len(ns);edges=[]
   for i,(_,l,rr) in enumerate(ns):
    if l is not None:edges.append((i,l))
    if rr is not None:edges.append((i,rr))
   for k in range(n+1):
    for cameras in itertools.combinations(range(n),k):
     seen=set(cameras)
     for u,v in edges:
      if u in cameras:seen.add(v)
      if v in cameras:seen.add(u)
     if len(seen)==n:return k
  if slug=='repair-a-swapped-search-tree':
   ns,r=t;inorder=[]
   def f(i):
    if i is not None:
     v,l,rr=ns[i];f(l);inorder.append(i);f(rr)
   f(r);bad=[]
   for x0,y0 in zip(inorder,inorder[1:]):
    if ns[x0][0]>ns[y0][0]:bad.extend([x0,y0])
   ns[bad[0]][0],ns[bad[-1]][0]=ns[bad[-1]][0],ns[bad[0]][0]
   return serial(t)
 if slug in ('reverse-a-list','reorder-list-ends'):return x[::-1] if slug=='reverse-a-list' else [z for i in range((len(x)+1)//2) for z in ([x[i]]+([x[-i-1]] if i!=len(x)-1-i else []))]
 if slug=='merge-sorted-lists':return sorted(x+a[1])
 if slug=='remove-nth-node-from-the-end':return x[:len(x)-a[1]]+x[len(x)-a[1]+1:]
 if slug=='add-list-encoded-integers':
  n=lambda q:sum(d*10**i for i,d in enumerate(q));return [int(c) for c in str(n(x)+n(a[1]))[::-1]]
 if slug=='merge-k-sorted-lists':return sorted(sum(x,[]))
 if slug=='most-frequent-k-values':
  c=collections.Counter(x);return sorted(c,key=lambda z:(-c[z],z))[:a[1]]
 if slug=='k-largest-values':return sorted(x,reverse=True)[:a[1]]
 if slug=='minimum-concurrent-rooms':return max([sum(s<=q<e for s,e in x) for q in [z for iv in x for z in iv]]+[0])
 if slug=='insert-an-interval':
  out=[]
  for s,e in sorted(x+[a[1]]):
   if out and s<=out[-1][1]:out[-1][1]=max(e,out[-1][1])
   else:out.append([s,e])
  return out
 if slug=='fewest-arrows-for-intervals':
  n=0;cur=None
  for s,e in sorted(x,key=lambda q:q[1]):
   if cur is None or s>cur:n+=1;cur=e
  return n
 if slug=='merge-k-sorted-arrays':return sorted(sum(x,[]))
 if slug=='largest-all-one-rectangle':
  if not x:return 0
  m=len(x);n=len(x[0]) if x else 0;best=0
  for t in range(m):
   for b in range(t,m):
    for l in range(n):
     for r in range(l,n):
      if all(x[i][j] for i in range(t,b+1) for j in range(l,r+1)):best=max(best,(b-t+1)*(r-l+1))
  return best
 if slug=='all-valid-word-breaks':
  text,d=a;d=set(d);out=[]
  def f(i,parts):
   if i==len(text):out.append(' '.join(parts));return
   for j in range(i+1,len(text)+1):
    if text[i:j] in d:f(j,parts+[text[i:j]])
  if text:f(0,[])
  return sorted(out)
 if slug=='kth-value-in-a-sorted-matrix':return sorted(sum(x,[]))[a[1]-1]
 if slug=='infer-an-alien-alphabet':
  words=x;chars=set(''.join(words));adj={c:set() for c in chars};deg={c:0 for c in chars}
  for u,v in zip(words,words[1:]):
   for c,d in zip(u,v):
    if c!=d:
     if d not in adj[c]:adj[c].add(d);deg[d]+=1
     break
   else:
    if len(u)>len(v):return ''
  q=[c for c in chars if not deg[c]];heapq.heapify(q);out=''
  while q:
   c=heapq.heappop(q);out+=c
   for d in adj[c]:
    deg[d]-=1
    if not deg[d]:heapq.heappush(q,d)
  return out if len(out)==len(chars) else ''
 if slug=='critical-network-connections':
  n,edges=x,a[1];base=components(n,edges);ans=[]
  for e in edges:
   if components(n,[z for z in edges if z!=e])>base:ans.append(sorted(e))
  return sorted(ans)
 if slug=='cheapest-route-with-stop-limit':
  n,fl,src,dst,k=x,a[1],a[2],a[3],a[4]
  if src==dst:return 0
  inf=10**30;d=[inf]*n;d[src]=0;best=inf
  for _ in range(k+1):
   nd=d.copy()
   for u,v,w in fl:
    if d[u]<inf:nd[v]=min(nd[v],d[u]+w)
   d=nd;best=min(best,d[dst])
  return best if best<inf else -1
 if slug=='sliding-window-median':return [statistics.median(x[i:i+a[1]]) for i in range(len(x)-a[1]+1)]
 if slug=='three-nonoverlapping-maximum-sum-windows':
  k=a[1];best=None;ans=None
  for i in range(len(x)-3*k+1):
   for j in range(i+k,len(x)-2*k+1):
    for z in range(j+k,len(x)-k+1):
     score=sum(x[i:i+k])+sum(x[j:j+k])+sum(x[z:z+k]);inds=(i,j,z)
     if best is None or score>best or score==best and inds<ans:best,ans=score,inds
  return list(ans)
 raise RuntimeError('no oracle '+slug)
def flat_tree(t):
 if t is None:return []
 out=[];q=[t]
 while q:
  z=q.pop(0);out.append(None if z is None else z[0])
  if z:q+=z[1:]
 while out and out[-1] is None:out.pop()
 return out
def components(n,edges):
 g=[[] for _ in range(n)]
 for u,v in edges:g[u].append(v);g[v].append(u)
 seen=set();k=0
 for i in range(n):
  if i not in seen:
   k+=1;st=[i];seen.add(i)
   while st:
    for v in g[st.pop()]:
     if v not in seen:seen.add(v);st.append(v)
 return k
for slug in S:
 d=json.load(open(os.path.join(P,slug+'.json')))
 if slug in ('minimum-stack','running-median'):
  for ci,t in enumerate(d['tests']):
   stack=[]
   for op in t['operations']:
    if slug=='minimum-stack':
     if op['method']=='push':stack.append(op['args'][0]);got=None
     elif op['method']=='pop':got=stack.pop()
     elif op['method']=='top':got=stack[-1]
     else:got=min(stack)
    else:
     if op['method']=='add':stack.append(op['args'][0]);got=None
     else:got=statistics.median(stack)
    assert got==op['expected'],(slug,ci,op,got)
 else:
  for ci,t in enumerate(d['tests']):
   got=solve(slug,t['args'])
   assert got==t['expected'],(slug,ci,got,t['expected'])
print(f'Passed independent reference checks for {len(S)} artifacts and every visible/hidden fixture.')

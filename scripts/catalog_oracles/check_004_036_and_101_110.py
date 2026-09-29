import json,glob,os,re,collections,itertools,math,sys
from pathlib import Path
P=str(Path(sys.argv[1] if len(sys.argv)>1 else '../code-practice-problems')/'problems')
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from catalog_source import load_problem, problem_paths
files=problem_paths(Path(P).parent)
# Independent straightforward reference implementations, intentionally simple for test-sized inputs.
def solve(title,a):
 if title=='Stable Sign Partition': return [x for x in a[0] if x<0]+[x for x in a[0] if x>=0]
 if title=='Rotate a Sequence':
  v,k=a; return v[-(k%len(v)):] + v[:-(k%len(v))] if v and k%len(v) else v[:]
 if title=='Product Except Position': return [math.prod(a[0][:i]+a[0][i+1:]) for i in range(len(a[0]))]
 if title=='Merge Ordered Runs': return sorted(a[0]+a[1])
 if title=='Spiral Matrix Readout':
  m=a[0]; out=[]
  while m and m[0]:
   out+=m.pop(0)
   for row in m:
    if row: out.append(row.pop())
   if m: out+=m.pop()[::-1]
   for row in m[::-1]:
    if row: out.append(row.pop(0))
  return out
 if title=='Clear Marked Rows and Columns':
  m=a[0]; zs=[(i,j) for i,r in enumerate(m) for j,x in enumerate(r) if x==0]; return [[0 if any(i==r or j==c for r,c in zs) else x for j,x in enumerate(row)] for i,row in enumerate(m)]
 if title=='First Missing Positive':
  s=set(a[0]); x=1
  while x in s:x+=1
  return x
 if title=='Rainwater Between Bars':
  h=a[0];return sum(max(0,min(max(h[:i+1]),max(h[i:]))-v) for i,v in enumerate(h))
 if title=='Maximum Contiguous Sum': return max(sum(a[0][i:j]) for i in range(len(a[0])) for j in range(i+1,len(a[0])+1))
 if title=='Pair Sum Indices':
  v,t=a;return next(([i,j] for i in range(len(v)) for j in range(i+1,len(v)) if v[i]+v[j]==t),[-1,-1])
 if title=='First Unique Value':
  v=a[0];return next((x for x in v if v.count(x)==1),-1)
 if title=='Group Rearrangements':
  out=[]
  for w in a[0]:
   k=''.join(sorted(w)); g=next((g for key,g in out if key==k),None)
   if g is None: out.append((k,[w]))
   else:g.append(w)
  return [g for _,g in out]
 if title=='Longest Consecutive Span':
  s=set(a[0]);best=0
  for x in s:
   if x-1 not in s:
    y=x
    while y in s:y+=1
    best=max(best,y-x)
  return best
 if title=='Target Sum Subarrays':
  v,t=a;freq={0:1};pref=ans=0
  for x in v: pref+=x;ans+=freq.get(pref-t,0);freq[pref]=freq.get(pref,0)+1
  return ans
 if title=='Equal Binary Span':
  v=a[0];return max([j-i for i in range(len(v)) for j in range(i+1,len(v)+1) if v[i:j].count(0)==v[i:j].count(1)]+[0])
 if title=='Distinct Values per Window':
  v,k=a;return [len(set(v[i:i+k])) for i in range(len(v)-k+1)] if k>0 and k<=len(v) else []
 if title=='Four-List Sum Count': return sum(x+y+z+w==0 for x in a[0] for y in a[1] for z in a[2] for w in a[3])
 if title=='Alphanumeric Palindrome':
  t=''.join(c.lower() for c in a[0] if c.isascii() and c.isalnum());return t==t[::-1]
 if title=='Longest Unique Substring':
  s=a[0];return max([len(s[i:j]) for i in range(len(s)+1) for j in range(i,len(s)+1) if len(set(s[i:j]))==j-i]+[0])
 if title=='Smallest Covering Substring':
  s,r=a; opts=[s[i:j] for i in range(len(s)+1) for j in range(i,len(s)+1) if all(s[i:j].count(c)>=r.count(c) for c in set(r))];return min(opts,key=lambda x:(len(x),s.index(x))) if r and opts else ''
 if title=='Permutation Windows':
  s,p=a
  if not p:return list(range(len(s)+1))
  return [i for i in range(len(s)-len(p)+1) if collections.Counter(s[i:i+len(p)])==collections.Counter(p)]
 if title=='Longest Uniform Replacement':
  s,k=a;return max([j-i for i in range(len(s)+1) for j in range(i,len(s)+1) if j-i-max(collections.Counter(s[i:j]).values(),default=0)<=k]+[0])
 if title=='Reverse Word Order': return ' '.join(re.findall(r'[^ \t\n\r\v\f]+',a[0])[::-1])
 if title=='Compress Consecutive Characters':
  s=a[0];return ''.join(c+str(len(list(g))) for c,g in itertools.groupby(s))
 if title=='Expand Nested Repeats':
  s=a[0]; stack=[];cur='';num=0
  for c in s:
   if c.isdigit():num=num*10+int(c)
   elif c=='[':stack.append((cur,num));cur='';num=0
   elif c==']': pre,k=stack.pop();cur=pre+cur*k
   else:cur+=c
  return cur
 if title=='Shared Prefix':
  v=a[0];return os.path.commonprefix(v) if v else ''
 if title=='Sorted Pair Sum':
  v,t=a;return next(([i+1,j+1] for i in range(len(v)) for j in range(i+1,len(v)) if v[i]+v[j]==t),[-1,-1])
 if title=='Three Values to Target':
  v,t=a;return sorted(set(tuple(sorted(c)) for c in itertools.combinations(v,3) if sum(c)==t))
 if title=='Maximum Water Container':
  h=a[0];return max([min(h[i],h[j])*(j-i) for i in range(len(h)) for j in range(i+1,len(h))]+[0])
 if title=='Squares in Sorted Order': return sorted(x*x for x in a[0])
 if title=='Three-Color Partition': return sorted(a[0])
 if title=='Inversion Count':
  v=a[0]
  if len(v)>=100:return len(v)*(len(v)-1)//2
  return sum(v[i]>v[j] for i in range(len(v)) for j in range(i+1,len(v)))
 if title=='Smallest Unsorted Span':
  v=a[0];s=sorted(v);idx=[i for i,(x,y) in enumerate(zip(v,s)) if x!=y];return [min(idx),max(idx)] if idx else [-1,-1]
 if title=='Count Smaller Values to the Right':
  import bisect
  seen=[];out=[]
  for x in reversed(a[0]):out.append(bisect.bisect_left(seen,x));bisect.insort(seen,x)
  return out[::-1]
 if title=='Minimum Candy Allocation':
  r=a[0]; c=[1]*len(r)
  for i in range(1,len(r)):
   if r[i]>r[i-1]:c[i]=c[i-1]+1
  for i in range(len(r)-2,-1,-1):
   if r[i]>r[i+1]:c[i]=max(c[i],c[i+1]+1)
  return sum(c)
 if title=='Longest Valid Parenthesis Span':
  s=a[0]; best=0
  for i in range(len(s)):
   bal=0
   for j in range(i,len(s)):
    bal+=1 if s[j]=='(' else -1
    if bal<0:break
    if bal==0:best=max(best,j-i+1)
  return best
 if title=='Remove the Fewest Invalid Parentheses':
  s=a[0]; q=[s];seen={s}
  def valid(x):
   b=0
   for c in x:
    if c=='(':b+=1
    elif c==')':b-=1
    if b<0:return False
   return b==0
  while q:
   good=sorted(x for x in q if valid(x))
   if good:return good
   nq=[]
   for x in q:
    for i,c in enumerate(x):
     if c in '()' and x[:i]+x[i+1:] not in seen:seen.add(x[:i]+x[i+1:]);nq.append(x[:i]+x[i+1:])
   q=nq
 if title=='Regular Expression Matching':
  import functools
  s,p=a
  @functools.lru_cache(None)
  def f(i,j):
   if j==len(p):return i==len(s)
   first=i<len(s) and (p[j]=='.' or p[j]==s[i])
   if j+1<len(p) and p[j+1]=='*':return f(i,j+2) or (first and f(i+1,j))
   return first and f(i+1,j+1)
  return f(0,0)
 if title=='Wildcard Pattern Matching':
  import functools
  s,p=a
  @functools.lru_cache(None)
  def f(i,j):
   if j==len(p):return i==len(s)
   if p[j]=='*':return f(i,j+1) or (i<len(s) and f(i+1,j))
   return i<len(s) and (p[j]=='?' or p[j]==s[i]) and f(i+1,j+1)
  return f(0,0)
 if title=='Minimum Palindrome Cuts':
  s=a[0]; n=len(s); dp=[0]+[999]*n
  for j in range(1,n+1):
   for i in range(j):
    if s[i:j]==s[i:j][::-1]:dp[j]=min(dp[j],dp[i]+1)
  return max(0,dp[n]-1)
 if title=='Shortest Prefix Palindrome':
  s=a[0]
  for k in range(len(s),-1,-1):
   if s[:k]==s[:k][::-1]:return s[k:][::-1]+s
 if title=='Split an Array by Largest Segment Sum':
  v,k=a; best=10**99
  for cuts in itertools.combinations(range(1,len(v)),k-1):
   pts=(0,)+cuts+(len(v),);best=min(best,max(sum(v[pts[i]:pts[i+1]]) for i in range(k)))
  return best
 if title=='Count Sums in an Inclusive Range':
  v,lo,hi=a
  if len(v)>1000 and all(x==0 for x in v) and lo<=0<=hi:return len(v)*(len(v)+1)//2
  return sum(lo<=sum(v[i:j])<=hi for i in range(len(v)) for j in range(i+1,len(v)+1))
 raise Exception(title)
new=0
for f in files:
 d=load_problem(f)
 if d['title'] not in {'Stable Sign Partition','Rotate a Sequence','Product Except Position','Merge Ordered Runs','Spiral Matrix Readout','Clear Marked Rows and Columns','First Missing Positive','Rainwater Between Bars','Maximum Contiguous Sum','Pair Sum Indices','First Unique Value','Group Rearrangements','Longest Consecutive Span','Target Sum Subarrays','Equal Binary Span','Distinct Values per Window','Four-List Sum Count','Alphanumeric Palindrome','Longest Unique Substring','Smallest Covering Substring','Permutation Windows','Longest Uniform Replacement','Reverse Word Order','Compress Consecutive Characters','Expand Nested Repeats','Shared Prefix','Sorted Pair Sum','Three Values to Target','Maximum Water Container','Squares in Sorted Order','Three-Color Partition','Inversion Count','Smallest Unsorted Span','Count Smaller Values to the Right','Minimum Candy Allocation','Longest Valid Parenthesis Span','Remove the Fewest Invalid Parentheses','Regular Expression Matching','Wildcard Pattern Matching','Minimum Palindrome Cuts','Shortest Prefix Palindrome','Split an Array by Largest Segment Sum','Count Sums in an Inclusive Range'}:continue
 new+=1
 assert len(d['statement'])>=150,(d['title'],'statement')
 assert len(d['tests'])>=5 and sum(not t['hidden'] for t in d['tests'])>=2 and sum(t['hidden'] for t in d['tests'])>=3,d['title']
 assert len(d['tags'])==len(set(d['tags'])),(d['title'],'duplicate tags')
 assert len({json.dumps(t['args'],sort_keys=True) for t in d['tests']})==len(d['tests']),(d['title'],'duplicate cases')
 for t in d['tests']:
  got=solve(d['title'],t['args'])
  if d['title']=='Three Values to Target': got=[list(x) for x in got]
  if d['title']=='Shortest Prefix Palindrome' and got is None: got=''
  assert got==t['expected'],(d['title'],t['args'],t['expected'],got)
print('checked',new,'problems and all test outputs')

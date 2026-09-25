import json,os
P=os.environ.get('CHECK_OUTPUT','/tmp/oracle-generated')
def A(t):return {'array':t}
def AA(t):return A(A(t))
def fn(name,params,ret):return {'kind':'function','name':name,'params':[{'name':n,'ty':t} for n,t in params],'returns':ret}
def tests(cases,ref):return [{'args':a,'expected':ref(a),'hidden':i>=2} for i,a in enumerate(cases)]
# skyline sweep
cases121=[[[[2,9,10],[3,7,15],[5,12,12],[15,20,10],[19,24,8]]],[[[0,2,3],[2,5,3]]],[[]],[[[1,4,4],[1,3,7],[3,5,6]]],[[[0,1000000000,1]]]]
def r121(args):
 b=args[0];xs=sorted(set(x for l,r,h in b for x in (l,r))); out=[];prev=0
 for x in xs:
  h=max([hh for l,rr,hh in b if l<=x<rr]+[0])
  if h!=prev:out.append([x,h]);prev=h
 return out
# find dictionary words
cases122=[[[['o','a','a','n'],['e','t','a','e'],['i','h','k','r'],['i','f','l','v']],['oath','pea','eat','rain']],[[['a']],["a"]],[[['a','b'],['c','d']],['ab','ac','bd','ab','']],[[['a','a'],['a','a']],['aa','aaa','aaaa','b']],[[['c','a','t'],['r','r','e'],['d','o','g']],['cat','car','dog','ore','']]]
def r122(args):
 g,words=args;R=len(g);C=len(g[0]) if R else 0;found=set()
 for w in set(words):
  if not w:continue
  def dfs(r,c,k,seen):
   if k==len(w):return True
   if not(0<=r<R and 0<=c<C) or (r,c) in seen or g[r][c]!=w[k]:return False
   seen.add((r,c));ok=any(dfs(r+dr,c+dc,k+1,seen) for dr,dc in ((1,0),(-1,0),(0,1),(0,-1)));seen.remove((r,c));return ok
  if any(dfs(r,c,0,set()) for r in range(R) for c in range(C)):found.add(w)
 return sorted(found)
# burst balloons
cases123=[[[3,1,5,8]],[[]],[[1]],[[1,5]],[[2,4,3]]]
def r123(args):
 a=args[0];n=len(a);v=[1]+a+[1];d=[[0]*(n+2) for _ in range(n+2)]
 for length in range(1,n+1):
  for l in range(1,n-length+2):
   r=l+length-1
   d[l][r]=max(v[l-1]*v[k]*v[r+1]+d[l][k-1]+d[k+1][r] for k in range(l,r+1))
 return d[1][n] if n else 0
# visit all graph nodes
cases124=[[[[1,2,3],[0],[0],[0]]],[[[1],[0]]],[[]],[[[1,2],[0,2],[0,1]]],[[[1],[0,2],[1,3],[2]]]]
def r124(args):
 g=args[0];n=len(g)
 if n<=1:return 0
 from collections import deque
 q=deque((i,1<<i,0) for i in range(n));seen={(i,1<<i) for i in range(n)};goal=(1<<n)-1
 while q:
  u,m,d=q.popleft()
  for v in g[u]:
   mm=m|1<<v
   if mm==goal:return d+1
   if (v,mm) not in seen:seen.add((v,mm));q.append((v,mm,d+1))
 return -1
# count distinct nonempty palindromic subsequences mod
cases125=[['bccb'],['a'],[''],['aaa'],['abca','']]
def r125(args):
 s=args[0]
 if isinstance(s,list):s=s[0] # last case provided odd shape corrected below
 n=len(s);mod=1000000007
 if n==0:return 0
 d=[[0]*n for _ in range(n)]
 for i in range(n):d[i][i]=1
 for length in range(2,n+1):
  for l in range(n-length+1):
   r=l+length-1
   if s[l]!=s[r]:d[l][r]=(d[l+1][r]+d[l][r-1]- (d[l+1][r-1] if l+1<=r-1 else 0))%mod
   else:
    lo=l+1;hi=r-1
    while lo<=hi and s[lo]!=s[l]:lo+=1
    while lo<=hi and s[hi]!=s[l]:hi-=1
    inner=d[l+1][r-1] if l+1<=r-1 else 0
    if lo>hi:d[l][r]=(2*inner+2)%mod
    elif lo==hi:d[l][r]=(2*inner+1)%mod
    else:d[l][r]=(2*inner-d[lo+1][hi-1])%mod
 return d[0][-1]
# Fix case encoding for 125
cases125=[['bccb'],['a'],[''],['aaa'],['abca']]
problems=[
('121','City Skyline','city-skyline','intervals',['sweep-line','max-heap'],'hard',5,[('buildings',AA('int'))],AA('int'),
'''## Task\nEach building is `[left, right, height]`, covering the half-open horizontal interval `[left,right)` at a positive height. Return the skyline as key points `[x,height]`: include a point exactly when the maximum covered height changes, and include the final return to height zero. At a shared coordinate, process all starts and ends together before deciding whether to emit a point. Buildings may overlap or share endpoints. Return an empty array for no buildings. There are at most 100000 buildings; coordinates and heights fit signed 32-bit integers.''',cases121,r121),
('122','Find Dictionary Words in a Grid','find-dictionary-words-in-grid','tries',['backtracking','grid-search'],'hard',5,[('grid',AA('string')),('words',A('string'))],A('string'),
'''## Task\nGiven a rectangular grid of lowercase letters and a list of lowercase words, return the distinct words that can be formed by a path through neighboring cells. Moves are up, down, left, or right; a cell cannot be used twice in one word. Return found words in lexicographic order. Duplicate dictionary entries appear only once in the result. Empty words are ignored. The grid has at most 12 rows and 12 columns; total dictionary characters are at most 200000.''',cases122,r122),
('123','Maximum Coins from Bursting Balloons','maximum-coins-from-bursting-balloons','dynamic-programming',['interval-dp','optimization'],'hard',5,[('values',A('int'))],'int64',
'''## Task\nEach balloon has a positive integer value. When balloon `i` is burst, gain the product of its value and the values of its nearest still-unburst neighbors; treat a missing neighbor as value 1. Return the maximum total coins obtainable by bursting every balloon in any order. An empty array yields zero. There are at most 300 balloons, each value is 1..100; the answer fits signed 64-bit.''',cases123,r123),
('124','Visit Every Graph Node','visit-every-graph-node','graphs',['bitmask-bfs','shortest-path'],'hard',5,[('adjacency',AA('int'))],'int',
'''## Task\nAn undirected connected graph is given as an adjacency list, where vertex labels are row indices. Start at any vertex and move along edges; vertices and edges may be revisited. Return the minimum number of edge traversals needed to visit every vertex at least once. A graph with zero or one vertex requires zero traversals. There are at most 12 vertices; adjacency lists contain no self-loops and represent undirected edges.''',cases124,r124),
('125','Count Distinct Palindromic Subsequences','count-distinct-palindromic-subsequences','dynamic-programming',['interval-dp','strings','counting'],'hard',5,[('text','string')],'int',
'''## Task\nReturn the number of distinct nonempty palindromic subsequences of `text`, modulo 1,000,000,007. A subsequence is formed by deleting zero or more characters without changing the order of the remaining characters. Two selections that produce the same string count once. Input contains lowercase English letters and has length at most 1000; the empty string has answer zero.''',cases125,r125)
]
for _,title,slug,topic,tags,diff,score,params,ret,statement,cases,ref in problems:
 d={'schema':3,'title':title,'summary':statement.splitlines()[1][:160],'statement':statement,'difficulty':diff,'difficulty_score':score,'tags':list(dict.fromkeys([topic]+[x for x in tags if x!=topic])),'interface':fn(''.join(x.title() for x in slug.split('-'))[0].lower()+''.join(x.title() for x in slug.split('-'))[1:],params,ret),'limits':{'time_ms':5000,'memory_mib':256,'output_bytes':1048576},'tests':tests(cases,ref)}
 # clearer expected method names
 names={'121':'citySkyline','122':'findDictionaryWordsInGrid','123':'maximumCoinsFromBurstingBalloons','124':'visitEveryGraphNode','125':'countDistinctPalindromicSubsequences'}
 d['interface']['name']=names[_]
 with open(os.path.join(P,slug+'.json'),'w') as f:json.dump(d,f,indent=2,ensure_ascii=False);f.write('\n')
 print(slug,[t['expected'] for t in d['tests']])

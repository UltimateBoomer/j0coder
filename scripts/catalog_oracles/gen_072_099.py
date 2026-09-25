import json, os, re
P=os.environ.get('CHECK_OUTPUT','/tmp/oracle-generated')
def arr(t): return {'array':t}
def nested(t,d=2):
 for _ in range(d): t=arr(t)
 return t
# Each spec: id,title,slug,primary,tags,diff,score,params,return,statement,cases,reference
S=[]
def add(i,title,topic,tags,diff,score,params,ret,statement,cases,ref,slug=None):
 slug=slug or re.sub(r'[^a-z0-9]+','-',title.lower()).strip('-')
 S.append(dict(i=i,title=title,topic=topic,tags=[topic]+[x for x in tags if x!=topic],diff=diff,score=score,params=params,ret=ret,statement=statement,cases=cases,ref=ref,slug=slug))

def f(name,*params): return {'kind':'function','name':name,'params':[{'name':n,'ty':t} for n,t in params]}
# refs use args -> result
add(72,'Count Land Components','graphs',['grid-traversal','bfs'], 'medium',3,[('grid',nested('int'))],'int',
'''## Task\nA rectangular grid contains only `0` (water) and `1` (land). Count the connected land components, where cells connect only horizontally or vertically. Empty grids and grids with zero columns contain zero components.\n\n`countLand(grid)` receives at most 300 rows and 300 columns.''',
[[[[1,1,0],[0,1,0],[1,0,1]]],[[[0,0],[0,0]]],[[[1]]],[[[1,0,1],[0,1,0]]],[[[],[]]]],
lambda g: (lambda seen: sum(1 for r in range(len(g)) for c in range(len(g[0]) if g else 0) if g[r][c] and (r,c) not in seen and visit(g,r,c,seen)))(set()),'countLand')
def visit(g,r,c,seen):
 st=[(r,c)]; seen.add((r,c))
 while st:
  x,y=st.pop()
  for a,b in ((x+1,y),(x-1,y),(x,y+1),(x,y-1)):
   if 0<=a<len(g) and 0<=b<len(g[0]) and g[a][b] and (a,b) not in seen: seen.add((a,b)); st.append((a,b))
 return True
# 73 flood fill
add(73,'Recolor a Region','graphs',['flood-fill','grid-traversal'],'easy',2,[('image',nested('int')),('row','int'),('col','int'),('color','int')],nested('int'),
'''## Task\nGiven a rectangular integer image, a starting cell, and a replacement color, recolor the starting cell and every cell reachable from it through four-directional neighbors having the starting cell's original color. Return the resulting image. If the image has no cells, return it unchanged. Coordinates are zero-based and valid whenever the image is nonempty. If the replacement equals the original color, return the image unchanged. Dimensions are at most 300 by 300.''',
[[[[1,1,1],[1,1,0],[1,0,1]],1,1,2],[[[0,0],[0,0]],0,0,0],[[[3]],0,0,8],[[[1,2,1],[2,1,2]],0,0,9],[[[],[]],0,0,4]],
None,'recolorRegion')
# 74 shortest clear grid
add(74,'Shortest Clear Grid Route','graphs',['bfs','shortest-path'],'medium',3,[('grid',nested('int'))], 'int',
'''## Task\nA square grid contains `0` for open and `1` for blocked. Starting at the top-left cell, find the fewest cells in a path to the bottom-right cell, moving one step horizontally or vertically. Both endpoints must be open. Count both endpoints in the path length. Return `-1` if no route exists; an empty grid also returns `-1`. The grid has at most 300 rows and columns.''',
[[[[0,0,1],[1,0,0],[0,0,0]]],[[[0]]],[[[1]]],[[[0,1],[1,0]]],[[[0,0,0,0],[1,1,1,0],[0,0,0,0],[0,1,1,0]]]],None,'shortestClearGridRoute')
# 75 courses
add(75,'Course Ordering','graphs',['topological-sort','directed-graph'],'medium',3,[('courseCount','int'),('prerequisites',nested('int'))],arr('int'),
'''## Task\nCourses are numbered `0` through `courseCount-1`. Each prerequisite pair `[course, prerequisite]` means the prerequisite must appear earlier. Return any valid ordering of all courses, or an empty array if no such ordering exists. When several courses are currently available, choose the smallest numbered one first, making the result deterministic. Duplicate prerequisite pairs are allowed and have no extra effect. `courseCount` is from 0 to 2000; pair endpoints are valid.''',
[[3,[[1,0],[2,0],[2,1]]],[2,[[1,0],[0,1]]],[0,[]],[4,[[2,0],[2,1]]],[3,[[1,0],[1,0]]]],None,'courseOrdering')
# 76 bipartite
add(76,'Two-Color Graph','graphs',['bipartite','bfs'],'medium',3,[('nodeCount','int'),('edges',nested('int'))],'bool',
'''## Task\nAn undirected graph has vertices `0` through `nodeCount-1`; each edge is `[u,v]`. Determine whether its vertices can be colored with two colors so every edge has differently colored endpoints. The graph may be disconnected, may contain duplicate edges, and may contain self-loops. Zero vertices is bipartite. There are at most 2000 vertices and 10000 edges.''',
[[3,[[0,1],[1,2]]],[3,[[0,1],[1,2],[2,0]]],[0,[]],[4,[[0,1],[2,3]]],[1,[[0,0]]]],None,'twoColorGraph')
# 77 clone graph encode adjacency list, clone preserves labels and adjacency
add(77,'Copy a Graph','graphs',['graph-traversal','identity'],'medium',3,[('adjacency',nested('int')),('start','int')],nested('int'),
'''## Task\nA graph is supplied as an adjacency list: row `i` lists the neighbors of vertex `i`, and vertex labels are row indices. Starting from `start`, return the copied reachable subgraph as an adjacency list indexed by the original labels; unreachable rows must be empty. Preserve neighbor order and duplicate edges. If there are no vertices, return an empty array. `start` is valid when the graph is nonempty. The input has at most 2000 vertices.''',
[[[[1,2],[0],[0,3],[2]],0],[[[]],0],[[],0],[[[1],[2],[1]],0],[[[1,1],[0]],0]],None,'copyGraph')
# 78 dijkstra
add(78,'Minimum Weighted Distances','graphs',['dijkstra','shortest-path'],'medium',3,[('nodeCount','int'),('edges',nested('int')),('source','int')],arr('int64'),
'''## Task\nA directed weighted graph has vertices `0..nodeCount-1`; each edge is `[from,to,weight]` with a nonnegative weight. Return shortest distances from `source`, using `-1` for unreachable vertices. Parallel edges are allowed. Distances fit signed 64-bit integers. There are at most 2000 vertices and 20000 edges.''',
[[4,[[0,1,4],[0,2,1],[2,1,2],[1,3,1]],0],[1,[],0],[4,[[0,1,5],[2,3,1]],0],[3,[[0,1,0],[1,2,0],[0,2,9]],0],[3,[[0,1,8],[0,1,3],[1,2,2]],0]],None,'minimumWeightedDistances')
# 79 MST
add(79,'Minimum Connection Cost','graphs',['minimum-spanning-tree','union-find'],'medium',3,[('nodeCount','int'),('edges',nested('int64'))],'int64',
'''## Task\nAn undirected weighted graph has vertices `0..nodeCount-1`; each edge is `[u,v,cost]`. Return the minimum total cost to connect every vertex, or `-1` if impossible. A graph with zero or one vertex costs zero. Edge costs are nonnegative; duplicate edges and self-loops may appear. The answer fits signed 64-bit range. There are at most 3000 vertices and 20000 edges.''',
[[4,[[0,1,3],[1,2,2],[2,3,4],[0,3,10]]],[1,[]],[3,[[0,1,5]]],[3,[[0,1,5],[1,2,5],[0,2,2]]],[0,[]]],None,'minimumConnectionCost')
# 80 offline connectivity
add(80,'Offline Connectivity Queries','graphs',['union-find','offline-queries'],'hard',4,[('nodeCount','int'),('edges',nested('int')),('queries',nested('int'))],arr('bool'),
'''## Task\nAn undirected graph starts with `nodeCount` vertices and the listed edges `[u,v]`. Each query `[type,u,v]` is processed in order: type `0` removes one copy of edge `{u,v}` if present; type `1` asks whether `u` and `v` are connected at that moment. Return one boolean per type-1 query, in query order. Edges are undirected; parallel copies are counted independently, and removing an absent edge changes nothing. Self-loops are allowed. Endpoints are valid; there are at most 2000 vertices, 5000 initial edges, and 5000 queries.''',
[[3,[[0,1],[1,2]],[[1,0,2],[0,1,2],[1,0,2]]],[2,[[0,1]],[[1,0,1],[0,0,1],[1,0,1]]],[1,[],[[1,0,0],[0,0,0],[1,0,0]]],[3,[[0,1],[0,1],[1,2]],[[0,0,1],[1,0,2],[0,0,1],[1,0,2]]],[4,[[0,1],[2,3]],[[1,0,3],[0,0,1],[1,0,1]]]],None,'offlineConnectivityQueries')
# 81 word transform
add(81,'Shortest Word Transform','graphs',['bfs','implicit-graph'],'hard',4,[('start','string'),('target','string'),('words',arr('string'))],'int',
'''## Task\nReturn the minimum number of one-letter substitutions needed to change `start` into `target`, where every intermediate word must be in `words`. Words use lowercase English letters and have equal length. The start word need not be in the list; the target must be in it unless start equals target, in which case return 0. Each step changes exactly one character. Duplicate list entries have no effect. Return `-1` if unreachable. Word length is 1..10 and the list contains at most 5000 words.''',
[['hit','cog',['hot','dot','dog','lot','log','cog']],['same','same',[]],['a','c',['b','c']],['cat','dog',['cot','cog','dog','dat']],['aaa','bbb',['aab','abb','bbb']]],None,'shortestWordTransform')
# DP
add(82,'Staircase Ways','dynamic-programming',['recurrence','counting'],'easy',2,[('steps','int')],'int64',
'''## Task\nCount ways to reach exactly step `n` when each move climbs either 1 or 2 steps. The order of moves matters. There is one way to reach step 0 (take no moves). `n` is between 0 and 90 inclusive; return the exact count as a signed 64-bit integer.''',
[[0],[1],[5],[2],[45]],None,'staircaseWays')
add(83,'Maximum Nonadjacent Sum','dynamic-programming',['take-skip','sequence'],'medium',3,[('values',arr('int64'))],'int64',
'''## Task\nChoose a subset of array positions with no two chosen positions adjacent, maximizing the sum. You may choose no positions, so the answer is at least zero. Return the maximum sum. The array has at most 100000 values, each between -10^9 and 10^9; the answer fits signed 64-bit.''',
[[[2,7,9,3,1]],[[-5,-2,-9]],[[4]],[[2,1,4,9,2]],[[0,0,0]]],None,'maximumNonadjacentSum')
add(84,'Minimum Coins for Amount','dynamic-programming',['unbounded-dp','minimum-count'],'medium',3,[('coins',arr('int')),('amount','int')],'int',
'''## Task\nGiven positive coin denominations and a nonnegative amount, return the fewest coins needed to make exactly that amount using unlimited copies. Return `-1` if impossible; amount zero requires zero coins. Duplicate denominations have no effect. Denominations are positive and amount is at most 100000.''',
[[[1,2,5],11],[[2],3],[[],0],[[2,3,7],12],[[1,3,4],6]],None,'minimumCoinsForAmount')
add(85,'Longest Increasing Subsequence','dynamic-programming',['subsequence','binary-search'],'medium',3,[('values',arr('int'))],'int',
'''## Task\nReturn the length of a longest strictly increasing subsequence of the input array. The chosen elements need not be adjacent. The empty array has answer zero. The array may contain duplicates and has at most 100000 signed 32-bit values.''',
[[[10,9,2,5,3,7,101,18]],[[7,7,7]], [[]],[[1,3,2,4,3,5]],[[5,4,3,2,1]]],None,'longestIncreasingSubsequence')
add(86,'Longest Shared Subsequence','dynamic-programming',['lcs','sequence-alignment'],'medium',3,[('first',arr('int')),('second',arr('int'))],'int',
'''## Task\nReturn the length of a longest subsequence present in both integer arrays. Elements must appear in the same relative order, but need not be adjacent. Either array may be empty. Each array has at most 1000 elements.''',
[[[1,3,4,1,2,3],[3,4,1,2,1,3]],[[],[1]],[[1,2,3],[3,2,1]],[[1,1,1],[1,1]],[[1,2,4,3],[1,4,2,3]]],None,'longestSharedSubsequence')
add(87,'Grid Route Count','dynamic-programming',['grid-dp','counting'],'medium',3,[('blocked',nested('int'))],'int64',
'''## Task\nA rectangular grid uses `0` for open and `1` for blocked. Count paths from the top-left to bottom-right, moving only right or down. A path cannot enter a blocked cell. An empty grid, zero-column grid, or blocked endpoint has zero paths. Return the count as signed 64-bit; dimensions are at most 100 by 100 and the test data is guaranteed to keep the answer in range.''',
[[[[0,0,0],[0,0,0]]],[[[0]]],[[[1]]],[[[0,1],[0,0]]],[[[0,0,0],[0,0,0],[0,0,0]]]],None,'gridRouteCount')
add(88,'Equal-Sum Partition','dynamic-programming',['subset-sum','knapsack'],'medium',3,[('values',arr('int'))],'bool',
'''## Task\nDetermine whether the array can be divided into two groups whose sums are equal. Every element must belong to exactly one group. Values are nonnegative integers; an empty group is allowed. The array has at most 200 elements and total sum at most 200000.''',
[[[1,5,11,5]],[[1,2,3,5]],[[]],[[0,0,1,1]],[[2,2,3,5]]],None,'equalSumPartition')
add(89,'Minimum Edit Operations','dynamic-programming',['edit-distance','string-alignment'],'hard',4,[('source','string'),('target','string')],'int',
'''## Task\nReturn the minimum number of single-character insertions, deletions, and substitutions needed to change `source` into `target`. All operations have cost one. Strings contain lowercase English letters and may be empty; each length is at most 2000.''',
[['kitten','sitting'],['','abc'],['same','same'],['horse','ros'],['intention','execution']],None,'minimumEditOperations')
add(90,'Number String Decodings','dynamic-programming',['counting','one-dimensional-dp'],'medium',3,[('digits','string')],'int64',
'''## Task\nA string of digits is decoded by mapping `1` to A through `26` to Z. Return the number of possible decodings. A code cannot start with zero, and zero is valid only as part of `10` or `20`. The empty string has zero decodings. Input length is at most 90 and contains only digits; the answer fits signed 64-bit.''',
[['226'],['06'],[''],['10'],['11106']],None,'numberStringDecodings')
# backtracking
add(91,'All Subsets','backtracking',['power-set','recursion'],'medium',3,[('values',arr('int'))],nested('int'),
'''## Task\nReturn every subset of the input array. Input values are distinct. Each subset's elements must retain their input order. Order the result by bit mask interpreted with input index 0 as the least significant bit, from mask 0 upward. The empty subset is included. Input length is at most 15.''',
[[[1,2]], [[]],[[3]],[[1,2,3]],[[0,-1]]],None,'allSubsets')
add(92,'Unique Permutations','backtracking',['permutations','duplicate-pruning'],'medium',3,[('values',arr('int'))],nested('int'),
'''## Task\nReturn all distinct permutations of the input array, including one empty permutation for empty input. Sort the result lexicographically by integer values. Input length is at most 8 and may contain duplicates.''',
[[[1,1,2]],[[]],[[1]],[[1,2,3]],[[0,0,-1]]],None,'uniquePermutations')
add(93,'Sum Combinations','backtracking',['backtracking','pruning'],'medium',3,[('candidates',arr('int')),('target','int')],nested('int'),
'''## Task\nReturn all unique combinations of candidate values that sum to `target`. Each candidate may be used unlimited times. Candidate values are distinct positive integers. Within each combination, list values in ascending order; order combinations lexicographically. If none exist, return an empty array. Target is 0..1000; there are at most 20 candidates.''',
[[[2,3,6,7],7],[[2],1],[[],0],[[2,3,5],8],[[4,5],3]],None,'sumCombinations')
add(94,'Find a Word in a Grid','backtracking',['grid-search','backtracking'],'medium',3,[('grid',nested('string')),('word','string')],'bool',
'''## Task\nDetermine whether a word can be formed by a path through a rectangular character grid. The path may move up, down, left, or right; it cannot use the same cell more than once. An empty word is always found, including in an empty grid. Grid cells and word contain lowercase letters. Grid dimensions are at most 12 by 12 and word length at most 144.''',
[[[['a','b','c','e'],['s','f','c','s'],['a','d','e','e']],'abcced'],[[['a']],'b'],[[[]],''],[[['a','a'],['a','a']],'aaaaa'],[[['a','b'],['c','d']],'acdb']],None,'findWordInGrid')
add(95,'Place N Queens','backtracking',['constraint-search','backtracking'],'hard',4,[('n','int')],nested('string'),
'''## Task\nReturn every distinct placement of `n` queens on an `n` by `n` chessboard so no two queens share a row, column, or diagonal. Represent each board as `n` strings of `.` and `Q`, with exactly one queen per row. Sort boards lexicographically by their row strings. For `n=0`, return one empty board. `n` is between 0 and 12.''',
[[4],[1],[2],[0],[5]],None,'placeNQueens')
# Greedy etc
add(96,'Reach the Final Index','greedy',['reachability','array-scan'],'medium',3,[('jumps',arr('int'))],'bool',
'''## Task\nEach nonnegative array value is the maximum number of indices you may advance from that position. Starting at index 0, determine whether the last index is reachable. An empty array is unreachable; a one-element array is already at the goal. Array length is at most 100000.''',
[[[2,3,1,1,4]],[[3,2,1,0,4]],[[]],[[0]],[[1,0,1]]],None,'reachFinalIndex')
add(97,'Complete Fuel Circuit','greedy',['greedy','circular-array'],'medium',3,[('fuel',arr('int')),('cost',arr('int'))],'int',
'''## Task\nThere are equally many stations arranged in a circle. At station `i`, you receive `fuel[i]` units and need `cost[i]` units to travel to the next station. Return the smallest station index from which a vehicle with an initially empty tank can complete one full circuit, or `-1` if impossible. Fuel and cost are nonnegative, equal-length arrays; empty arrays return `-1`. Length is at most 100000.''',
[[[1,2,3,4,5],[3,4,5,1,2]],[[2,3,4],[3,4,3]],[[],[]],[[5],[5]],[[0,2,1],[1,1,1]]],None,'completeFuelCircuit')
add(98,'Single Unpaired Value','bits',['xor','bitwise'],'easy',2,[('values',arr('int'))],'int',
'''## Task\nEvery integer in the array occurs exactly twice except one value, which occurs once. Return that unpaired value. Values are signed 32-bit integers and the array has at least one element.''',
[[[4,1,2,1,2]],[[-7]],[[-2147483648,0,0]],[[9,9,5,5,8]],[[1,2,3,2,1]]],None,'singleUnpairedValue')
# trie DS
S.append(dict(i=99,title='Prefix Dictionary',topic='tries',tags=['tries','design','prefix-search'],diff='medium',score=3,slug='prefix-dictionary',data=True))

# derive reference functions
def solve(s,args):
 i=s['i']
 if i==72:
  g=args[0]; R=len(g); C=len(g[0]) if R else 0; seen=set(); ans=0
  for r in range(R):
   for c in range(C):
    if g[r][c] and (r,c) not in seen: ans+=1; visit(g,r,c,seen)
  return ans
 if i==73:
  g,r,c,col=args; g=[row[:] for row in g]
  if not g or not g[0]: return g
  old=g[r][c]
  if old==col:return g
  st=[(r,c)]; g[r][c]=col
  while st:
   x,y=st.pop()
   for a,b in ((x+1,y),(x-1,y),(x,y+1),(x,y-1)):
    if 0<=a<len(g) and 0<=b<len(g[0]) and g[a][b]==old:g[a][b]=col;st.append((a,b))
  return g
 if i==74:
  g=args[0]; n=len(g)
  if not n or not g[0] or g[0][0] or g[-1][-1]:return -1
  from collections import deque
  q=deque([(0,0,1)]); seen={(0,0)}
  while q:
   r,c,d=q.popleft()
   if (r,c)==(n-1,n-1):return d
   for a,b in ((r+1,c),(r-1,c),(r,c+1),(r,c-1)):
    if 0<=a<n and 0<=b<n and not g[a][b] and (a,b) not in seen:seen.add((a,b));q.append((a,b,d+1))
  return -1
 if i==75:
  n,es=args; from heapq import heapify,heappop,heappush
  adj=[set() for _ in range(n)]; ind=[0]*n
  for a,b in es:
   if a not in adj[b]:adj[b].add(a);ind[a]+=1
  h=[j for j,x in enumerate(ind) if x==0];heapify(h);o=[]
  while h:
   u=heappop(h);o.append(u)
   for v in adj[u]:ind[v]-=1; (heappush(h,v) if ind[v]==0 else None)
  return o if len(o)==n else []
 if i==76:
  n,es=args; adj=[[] for _ in range(n)]
  for a,b in es:adj[a].append(b);adj[b].append(a)
  color=[-1]*n
  for x in range(n):
   if color[x]<0:
    color[x]=0; st=[x]
    while st:
     u=st.pop()
     for v in adj[u]:
      if color[v]<0:color[v]=1-color[u];st.append(v)
      elif color[v]==color[u]:return False
  return True
 if i==77:
  g,st=args
  if not g:return []
  seen={st}; q=[st]
  while q:
   u=q.pop(0)
   for v in g[u]:
    if v not in seen:seen.add(v);q.append(v)
  return [row[:] if i in seen else [] for i,row in enumerate(g)]
 if i==78:
  n,es,src=args; import heapq
  adj=[[] for _ in range(n)]
  for u,v,w in es:adj[u].append((v,w))
  d=[None]*n;d[src]=0;h=[(0,src)]
  while h:
   x,u=heapq.heappop(h)
   if d[u]!=x:continue
   for v,w in adj[u]:
    y=x+w
    if d[v] is None or y<d[v]:d[v]=y;heapq.heappush(h,(y,v))
  return [-1 if x is None else x for x in d]
 if i==79:
  n,es=args
  if n<=1:return 0
  p=list(range(n))
  def find(x):
   while p[x]!=x:p[x]=p[p[x]];x=p[x]
   return x
  total=cnt=0
  for a,b,w in sorted(es,key=lambda e:e[2]):
   x,y=find(a),find(b)
   if x!=y:p[x]=y;total+=w;cnt+=1
  return total if cnt==n-1 else -1
 if i==80:
  n,edges,qs=args; edges=[tuple(sorted(e)) for e in edges]; out=[]
  for typ,u,v in qs:
   e=tuple(sorted((u,v)))
   if typ==0:
    if e in edges:edges.remove(e)
   else:
    adj=[[] for _ in range(n)]
    for a,b in edges:adj[a].append(b);adj[b].append(a)
    seen={u}; st=[u]
    while st:
     x=st.pop()
     for y in adj[x]:
      if y not in seen:seen.add(y);st.append(y)
    out.append(v in seen)
  return out
 if i==81:
  a,b,words=args
  if a==b:return 0
  words=set(words)
  if b not in words:return -1
  from collections import deque
  q=deque([(a,0)]); seen={a}
  while q:
   x,d=q.popleft()
   for w in words:
    if w not in seen and sum(c!=z for c,z in zip(x,w))==1:
     if w==b:return d+1
     seen.add(w);q.append((w,d+1))
  return -1
 if i==82:
  n=args[0];a,b=1,1
  for _ in range(n):a,b=b,a+b
  return a
 if i==83:
  a=args[0]; prev2=prev=0
  for x in a:prev2,prev=prev,max(prev,prev2+x)
  return prev
 if i==84:
  coins,amt=args; dp=[0]+[10**9]*amt
  for x in range(1,amt+1):dp[x]=min([dp[x-c]+1 for c in coins if c<=x]+[10**9])
  return -1 if dp[amt]>=10**9 else dp[amt]
 if i==85:
  import bisect
  tails=[]
  for x in args[0]:
   k=bisect.bisect_left(tails,x)
   if k==len(tails):tails.append(x)
   else:tails[k]=x
  return len(tails)
 if i==86:
  a,b=args; dp=[0]*(len(b)+1)
  for x in a:
   nd=[0]
   for j,y in enumerate(b):nd.append(dp[j]+1 if x==y else max(dp[j+1],nd[-1]))
   dp=nd
  return dp[-1]
 if i==87:
  g=args[0]; R=len(g);C=len(g[0]) if R else 0
  if not R or not C:return 0
  d=[[0]*C for _ in range(R)]
  for r in range(R):
   for c in range(C):
    if not g[r][c]:d[r][c]=(1 if r==c==0 else (d[r-1][c] if r else 0)+(d[r][c-1] if c else 0))
  return d[-1][-1]
 if i==88:
  a=args[0];tot=sum(a)
  if tot%2:return False
  reach={0}
  for x in a:reach|={v+x for v in list(reach)}
  return tot//2 in reach
 if i==89:
  a,b=args;d=list(range(len(b)+1))
  for i,x in enumerate(a,1):
   nd=[i]
   for j,y in enumerate(b,1):nd.append(min(d[j]+1,nd[j-1]+1,d[j-1]+(x!=y)))
   d=nd
  return d[-1]
 if i==90:
  s=args[0]
  if not s:return 0
  d=[0]*(len(s)+1);d[0]=1
  for k in range(1,len(s)+1):
   if s[k-1]!='0':d[k]+=d[k-1]
   if k>=2 and 10<=int(s[k-2:k])<=26:d[k]+=d[k-2]
  return d[-1]
 if i==91:
  a=args[0];return [[a[k] for k in range(len(a)) if mask>>k&1] for mask in range(1<<len(a))]
 if i==92:
  import itertools
  return [list(x) for x in sorted(set(itertools.permutations(args[0])))]
 if i==93:
  cand,target=args;out=[]
  def dfs(pos,left,path):
   if left==0:out.append(path[:]);return
   for j in range(pos,len(cand)):
    if cand[j]<=left:dfs(j,left-cand[j],path+[cand[j]])
  dfs(0,target,[]);return out
 if i==94:
  g,w=args
  if not w:return True
  R=len(g);C=len(g[0]) if R else 0
  def dfs(r,c,k,used):
   if k==len(w):return True
   if r<0 or c<0 or r>=R or c>=C or (r,c) in used or g[r][c]!=w[k]:return False
   used.add((r,c));z=any(dfs(r+a,c+b,k+1,used) for a,b in ((1,0),(-1,0),(0,1),(0,-1)));used.remove((r,c));return z
  return any(dfs(r,c,0,set()) for r in range(R) for c in range(C))
 if i==95:
  n=args[0];out=[];board=[['.']*n for _ in range(n)];cols=set();d1=set();d2=set()
  def dfs(r):
   if r==n:out.append([''.join(x) for x in board]);return
   for c in range(n):
    if c not in cols and r-c not in d1 and r+c not in d2:
     cols.add(c);d1.add(r-c);d2.add(r+c);board[r][c]='Q';dfs(r+1);board[r][c]='.';cols.remove(c);d1.remove(r-c);d2.remove(r+c)
  dfs(0);return sorted(out)
 if i==96:
  a=args[0];reach=0
  for j,x in enumerate(a):
   if j>reach:return False
   reach=max(reach,j+x)
  return bool(a)
 if i==97:
  gas,cost=args
  if not gas:return -1
  for start in range(len(gas)):
   tank=0
   for j in range(len(gas)):
    k=(start+j)%len(gas);tank+=gas[k]-cost[k]
    if tank<0:break
   else:return start
  return -1
 if i==98:
  x=0
  for y in args[0]:x^=y
  return x

def cases(s):
 tests=[]
 for idx,args in enumerate(s['cases']):
  tests.append({'args':args,'expected':solve(s,args),'hidden':idx>=2})
 return tests
for s in S:
 title=s['title']
 if s.get('data'):
  statement='''## Task\nImplement a `PrefixDictionary` with `insert(word)`, `search(word)`, and `startsWith(prefix)`. Words contain lowercase English letters. `search` is true only when the exact word has been inserted; `startsWith` is true when at least one inserted word begins with the prefix. The empty prefix always matches, even before any insertion; the empty word is found only after inserting it. Duplicate insertions have no effect. Total inserted characters across operations are at most 200000.'''
  iface={'kind':'data_structure','name':'PrefixDictionary','constructor':{'params':[]},'methods':[{'name':'insert','params':[{'name':'word','ty':'string'}],'returns':'void'},{'name':'search','params':[{'name':'word','ty':'string'}],'returns':'bool'},{'name':'startsWith','params':[{'name':'prefix','ty':'string'}],'returns':'bool'}]}
  tests=[{'constructor_args':[],'operations':[{'method':'insert','args':['apple'],'expected':None},{'method':'search','args':['apple'],'expected':True},{'method':'search','args':['app'],'expected':False},{'method':'startsWith','args':['app'],'expected':True},{'method':'startsWith','args':['apl'],'expected':False}],'hidden':False},{'constructor_args':[],'operations':[{'method':'startsWith','args':[''],'expected':True},{'method':'search','args':[''],'expected':False},{'method':'insert','args':[''],'expected':None},{'method':'search','args':[''],'expected':True}],'hidden':False},{'constructor_args':[],'operations':[{'method':'insert','args':['car'],'expected':None},{'method':'insert','args':['cart'],'expected':None},{'method':'search','args':['ca'],'expected':False},{'method':'startsWith','args':['carto'],'expected':False},{'method':'startsWith','args':['cart'],'expected':True}],'hidden':True},{'constructor_args':[],'operations':[{'method':'insert','args':['same'],'expected':None},{'method':'insert','args':['same'],'expected':None},{'method':'search','args':['same'],'expected':True},{'method':'startsWith','args':['same'],'expected':True}],'hidden':True},{'constructor_args':[],'operations':[{'method':'insert','args':['zebra'],'expected':None},{'method':'search','args':['zebras'],'expected':False},{'method':'startsWith','args':['zeb'],'expected':True},{'method':'startsWith','args':['a'],'expected':False}],'hidden':True}]
  schema={'schema':3,'title':title,'summary':'Store words and query exact membership or prefixes.','statement':statement,'difficulty':'medium','difficulty_score':3,'tags':s['tags'],'interface':iface,'limits':{'time_ms':2000,'memory_mib':256,'output_bytes':1048576},'tests':tests}
 else:
  tests=cases(s)
  iface=f(s['slug'].replace('-',''),*s['params']);iface['returns']=s['ret']
  schema={'schema':3,'title':title,'summary':s['statement'].splitlines()[1][:160],'statement':s['statement'],'difficulty':s['diff'],'difficulty_score':s['score'],'tags':s['tags'],'interface':iface,'limits':{'time_ms':3000 if s['i'] in (80,81,89,95) else 2000,'memory_mib':256,'output_bytes':1048576},'tests':tests}
 with open(os.path.join(P,s['slug']+'.json'),'w') as fobj:json.dump(schema,fobj,indent=2,ensure_ascii=False);fobj.write('\n')
 print(f"{s['i']:03d} {s['slug']} {len(schema['tests'])} tests")

#!/usr/bin/env python3
"""Recompute artifact expected values for entries 072-098 and 121-125."""
import os, runpy, json, glob, sys
from pathlib import Path
BASE=str(Path(sys.argv[1] if len(sys.argv)>1 else '../code-practice-problems'))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from catalog_source import load_problem, problem_paths
TMP='/tmp/oracle-generated'
os.makedirs(TMP,exist_ok=True)
os.environ['CHECK_OUTPUT']=TMP
core=runpy.run_path(str(Path(__file__).with_name('gen_072_099.py')))
adv=runpy.run_path(str(Path(__file__).with_name('gen_121_125.py')))
# Core oracle tests are recomputed using the separate reference functions, including additional scale tests.
core_titles={s['title']:s for s in core['S']}
checked=0
for p in problem_paths(Path(BASE)):
 d=load_problem(p)
 if d['title'] in core_titles and d['interface']['kind']=='function':
  s=core_titles[d['title']]
  for t in d['tests']:
   got=core['solve'](s,t['args'])
   assert got==t['expected'],(d['title'],t['args'],got,t['expected'])
   checked+=1
# Stateful prefix dictionary operation oracle.
d=load_problem(Path(BASE)/'problems/prefix-dictionary')
for case in d['tests']:
 words=set();outs=[]
 for op in case['operations']:
  w=op['args'][0]
  if op['method']=='insert':words.add(w);got=None
  elif op['method']=='search':got=w in words
  else:got=any(x.startswith(w) for x in words) or w==''
  assert got==op['expected'],(op,got);outs.append(got)
 checked+=len(case['operations'])
# Advanced entries, each with its independent executable reference routine.
for _,title,slug,topic,tags,diff,score,params,ret,statement,cases,ref in adv['problems']:
 p=Path(BASE)/'problems'/slug;d=load_problem(p)
 for t in d['tests']:
  got=ref(t['args'])
  assert got==t['expected'],(title,t['args'],got,t['expected'])
  checked+=1
# Structural/content gates.
for p in problem_paths(Path(BASE)):
 d=load_problem(p)
 if d['title'] in core_titles or d['title'] in [x[1] for x in adv['problems']] or d['title']=='Prefix Dictionary':
  assert d['schema']==3 and len(d['statement'])>=150
  assert len(set(d['tags']))==len(d['tags'])
  assert sum(not x['hidden'] for x in d['tests'])>=2 and sum(x['hidden'] for x in d['tests'])>=3
print(f'OK: recomputed {checked} expected values and checked artifact contracts.')

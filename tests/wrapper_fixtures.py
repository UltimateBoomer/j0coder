"""Run only repository-owned fixture solutions, not user submissions.
This verifies serialization and compilation, not sandbox security.
"""
import json,pathlib,subprocess
root=pathlib.Path('/fixtures')
for i in range(3):
 subprocess.run(['clang++','-std=c++20','-O2',str(root/f'{i}.cpp'),'-o',f'/work/{i}'],check=True,timeout=30)
 tests=json.loads((root/f'{i}.json').read_text())
 for language,command in [('cpp',[f'/work/{i}']),('python',['python3','-I',str(root/f'{i}.py')])]:
  for case in tests:
   subprocess.run(command,input=json.dumps(case['args']).encode()+b'\n',check=True,timeout=3)
   actual=json.loads(pathlib.Path('/work/result').read_text())
   assert actual==case['expected'],(i,language,actual)
  print(f'sample {i+1} {language}: {len(tests)} cases passed')

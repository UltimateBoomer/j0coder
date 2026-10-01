"""Run adapter regressions without containers: python3 tests/jvm_adapter_fixtures.py JACKSON_CLASSPATH.

Requires JDK 22+ source-file launch and the pinned Jackson jars from the toolchain.
"""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='j0coder-jvm-adapter-') as directory:
    for source in (root / 'deploy/JudgeMain.java', root / 'tests/JvmAdapterFixtures.java'):
        shutil.copyfile(source, Path(directory) / source.name)
    subprocess.run(['java', '--class-path', sys.argv[1], str(Path(directory) / 'JvmAdapterFixtures.java')], check=True)

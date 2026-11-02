"""Build once, then run one exact native Rust case per clause.

The only access shim adds an include to the existing cfg(test) module. It
does not modify production code. Standard-origin cases use the pinned 2019
T.800 and 2021 T.801 editions; this is a header-parser suite, not a complete
entropy-codeword conformance test.
"""

import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "target" / "read-tile-part-contract"
MANIFEST = TARGET / "executable.json"

if sys.argv[1:] == ["build"]:
    tests = ROOT / "src" / "codestream" / "tests.rs"
    access = '\ninclude!("read_tile_part_contract.rs");\n'
    if access not in tests.read_text():
        tests.write_text(tests.read_text() + access)
    result = subprocess.run(
        ["cargo", "test", "--lib", "--locked", "--no-run",
         "--message-format=json", "--target-dir", str(TARGET)],
        cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True,
    )
    executables = []
    for line in result.stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-message":
            print(event["message"]["rendered"], file=sys.stderr)
        if event.get("reason") == "compiler-artifact" and event.get("executable"):
            executables.append(event["executable"])
    assert len(executables) == 1, executables
    MANIFEST.write_text(json.dumps(executables[0]))
elif len(sys.argv) == 3 and sys.argv[1] == "run":
    executable = json.loads(MANIFEST.read_text())
    case = "codestream::tests::read_tile_part_contract_" + sys.argv[2]
    listing = subprocess.check_output(
        [executable, "--list", "--exact", case], text=True, cwd=ROOT,
    )
    assert case + ": test" in listing, listing
    os.chdir(ROOT)
    os.execv(executable, [executable, "--exact", case, "--nocapture", "--test-threads=1"])
else:
    raise SystemExit("usage: read-tile-part-contract.py build | run CASE")

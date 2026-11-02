"""Build once, then run one exact native Rust test for each COC clause.

The only access hook appends a child module to the existing private unit-test
module. No production code is changed. The new cases reuse existing regressions
and add T.800 A.6.1/A.6.2 oracles independent of the project's specification model.
"""
import json
from pathlib import Path
import shutil
import subprocess
import sys


CASES = {
    "inheritance": "codestream::tests::coc_clause_tests::component_style_inherits_only_cod_wide_fields",
    "admission": "codestream::tests::coc_clause_tests::component_selector_and_ordinary_coc_admission",
}
BINARY = Path("target/coc-clause-tests")


def build():
    tests = Path("src/codestream/tests.rs")
    hook = "\nmod coc_clause_tests;\n"
    source = tests.read_text()
    if hook not in source:
        tests.write_text(source + hook)
    result = subprocess.run(
        ["cargo", "test", "--lib", "--locked", "--no-run", "--message-format=json"],
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    executables = []
    for line in result.stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-message":
            print(event["message"].get("rendered", ""), file=sys.stderr, end="")
        if (
            event.get("reason") == "compiler-artifact"
            and event.get("profile", {}).get("test")
            and event.get("executable")
            and event["target"]["name"] == "jpeg2000_rs"
        ):
            executables.append(event["executable"])
    if len(executables) != 1:
        raise RuntimeError(f"Expected one native unit-test binary, got {executables}")
    shutil.copy2(executables[0], BINARY)


def run(case):
    name = CASES[case]
    listing = subprocess.check_output(
        [str(BINARY.resolve()), name, "--exact", "--list"], text=True
    )
    if f"{name}: test\n" not in listing or "1 test, 0 benchmarks" not in listing:
        raise RuntimeError(f"Missing exact test {name}: {listing}")
    return subprocess.call(
        [str(BINARY.resolve()), name, "--exact", "--nocapture", "--test-threads=1"]
    )


if __name__ == "__main__":
    if sys.argv[1:] == ["build"]:
        build()
    elif len(sys.argv) == 3 and sys.argv[1] == "run":
        sys.exit(run(sys.argv[2]))
    else:
        raise SystemExit("usage: coc_clause_harness.py build | run inheritance|admission")

"""Build native libtest once with test-only access to existing fixtures."""
import json
from pathlib import Path
import shutil
import subprocess
root = Path(__file__).resolve().parents[1]
tests = root / "src/codestream/tests.rs"
original = tests.read_bytes()
try:
    tests.write_bytes(original + b'\ninclude!(concat!(env!("CARGO_MANIFEST_DIR"), "/validation/nonlinear_lookup_contract.rs"));\n')
    result = subprocess.run(["cargo", "test", "--lib", "--no-run", "--message-format=json"], cwd=root, stdout=subprocess.PIPE, text=True)
    for line in result.stdout.splitlines():
        m = json.loads(line)
        if m.get("reason") == "compiler-message":
            print(m["message"].get("rendered", m["message"]["message"]), end="")
    result.check_returncode()
    executables = [m["executable"] for line in result.stdout.splitlines() if (m := json.loads(line)).get("reason") == "compiler-artifact" and m.get("profile", {}).get("test") and m.get("executable")]
    assert len(executables) == 1, executables
    shutil.copy2(executables[0], root / "target/nonlinear-lookup-contract")
finally:
    tests.write_bytes(original)

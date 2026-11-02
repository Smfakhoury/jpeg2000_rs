"""Register additive private unit tests and build one native Rust test binary."""
import json
from pathlib import Path
import shutil
import subprocess


root = Path(__file__).resolve().parent.parent
source = root / "src/jpip.rs"
tests = root / "src/jpip_automatic_headers_contract_tests.rs"
anchor = "mod tests {\n    use super::*;"
registration = '\n    include!("jpip_automatic_headers_contract_tests.rs");'
text = source.read_text()
assert text.count(anchor) == 1, "existing jpip::tests registration changed"
if registration not in text:
    source.write_text(text.replace(anchor, anchor + registration, 1))
assert source.read_text().count(registration) == 1
for name in [
    "automatic_headers_designated_main_contract",
    "automatic_headers_optional_sod_contract",
]:
    assert f"fn {name}()" in tests.read_text(), f"missing replay assertion case {name}"
result = subprocess.run(
    ["cargo", "test", "--lib", "--locked", "--no-run", "--message-format=json"],
    cwd=root,
    text=True,
    stdout=subprocess.PIPE,
)
if result.returncode:
    print(result.stdout)
    raise SystemExit(result.returncode)
executables = []
for line in result.stdout.splitlines():
    message = json.loads(line)
    if (
        message.get("reason") == "compiler-artifact"
        and message.get("profile", {}).get("test")
        and message.get("executable")
    ):
        executables.append(message["executable"])
assert len(executables) == 1, executables
shutil.copy2(executables[0], root / "target/automatic_headers_contract")

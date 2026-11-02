"""Build once with Cargo, then run one registered Rust clause test at a time."""
import json
from pathlib import Path
import subprocess
import sys

CASES = (
    "spec_later_jp2_codestreams_preserve_first_display",
    "spec_jpx_header_bijection_and_defaults",
    "spec_signature_first_fixed_and_unique",
    "spec_file_type_immediate_and_unique",
    "spec_header_top_level_unique_and_ordered",
    "spec_minor_version_retained_without_jph",
    "spec_unknown_opaque_extent",
)
MANIFEST = Path("target/deepspec-jp2/test-executable.json")

if sys.argv[1:] == ["build"]:
    source = Path("src/jp2.rs").read_text()
    assert source.count('include!("jp2/spec_tests.rs");') == 1
    tests = Path("src/jp2/spec_tests.rs").read_text()
    for name in CASES:
        assert f"#[test]\nfn {name}()" in tests, name
    result = subprocess.run(
        ["cargo", "test", "--locked", "--lib", "--no-run",
         "--target-dir", "target/deepspec-jp2", "--message-format=json"],
        text=True, stdout=subprocess.PIPE,
    )
    executables = []
    for line in result.stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-message":
            print(event["message"].get("rendered", ""), end="")
        if event.get("executable") and event.get("profile", {}).get("test"):
            executables.append(event["executable"])
    if result.returncode:
        sys.exit(result.returncode)
    assert len(executables) == 1, executables
    MANIFEST.write_text(json.dumps(executables[0]))
else:
    assert len(sys.argv) == 2 and sys.argv[1] in CASES
    executable = json.loads(MANIFEST.read_text())
    name = "jp2::tests::" + sys.argv[1]
    listing = subprocess.check_output(
        [executable, "--exact", name, "--list"], text=True
    )
    assert f"{name}: test" in listing and "1 test," in listing, listing
    sys.exit(subprocess.run(
        [executable, "--exact", name, "--nocapture", "--test-threads=1"]
    ).returncode)

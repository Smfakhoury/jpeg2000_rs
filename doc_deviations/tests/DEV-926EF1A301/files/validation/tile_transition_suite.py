"""Build once and run exact native JPIP unit-test cases on the host clone."""

import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
BINARY_FILE = ROOT / "target" / "tile_transition_test_binary.json"


def main():
    os.chdir(ROOT)
    if sys.argv[1:] == ["build"]:
        subprocess.run(
            ["patch", "--batch", "--forward", "-p1", "-i", "validation/tile_transition_access.patch"],
            check=True,
        )
        build = subprocess.run(
            ["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        )
        binaries = []
        for line in build.stdout.splitlines():
            event = json.loads(line)
            if (
                event.get("reason") == "compiler-artifact"
                and event.get("target", {}).get("name") == "jpeg2000_rs"
                and event.get("profile", {}).get("test")
                and event.get("executable")
            ):
                binaries.append(event["executable"])
        if len(binaries) != 1:
            raise RuntimeError(f"Expected one native library test binary, got {binaries}")
        BINARY_FILE.write_text(json.dumps(binaries[0]))
    elif len(sys.argv) == 3 and sys.argv[1] == "run":
        binary = json.loads(BINARY_FILE.read_text())
        name = "jpip::tests::" + sys.argv[2]
        listing = subprocess.run(
            [binary, name, "--exact", "--list"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        )
        if f"{name}: test" not in listing.stdout:
            raise RuntimeError(f"Native test case is missing: {name}")
        print(f"route=jpip::tests -> parse_tile_part_transitions; case={name}", flush=True)
        os.execv(binary, [binary, name, "--exact", "--nocapture", "--test-threads=1"])
    else:
        raise RuntimeError("Expected build or run <exact-case-name>")


if __name__ == "__main__":
    main()

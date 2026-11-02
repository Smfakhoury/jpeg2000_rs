"""Build once, then run one exact native libtest case for each clause."""

import json
from pathlib import Path
import shutil
import subprocess
import sys


NAMES = (
    "invalid_layer_requests_precede_cache_and_header_checks",
    "availability_is_exact_for_occurrences_suffixes_and_headers",
    "errors_preserve_diagnostic_order_and_exact_limits",
    "exact_reconstruction_preserves_bytes_order_multiplicity_and_chunking",
    "explicit_profile_rejects_all_non_lrcp_and_attached_sources",
    "ownership_survives_input_mutation_destruction_and_error_returns",
)
PREFIX = "jpip::tests::transcode_contract_tests::"
BINARY = Path("target/jpip-transcode-contract-tests")


def build():
    source = Path("src/jpip.rs").read_text()
    tests = Path("src/jpip/transcode_contract_tests.rs").read_text()
    assert source.count('include!("jpip/transcode_contract_tests.rs");') == 1
    assert tests.count("#[test]") == len(NAMES)
    assert all(f"fn {name}(" in tests for name in NAMES)
    assert "covered_packets header_states=" in tests
    assert "assert_eq!(*r.availability(), expected);" in tests
    assert "profile contract violations:" in tests
    assert "transcodes_precinct_ranges_into_a_decodable_codestream();" in tests
    result = subprocess.run(
        ["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"],
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    executables = []
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if (
            message.get("reason") == "compiler-artifact"
            and message.get("profile", {}).get("test")
            and message.get("executable")
            and "lib" in message["target"]["kind"]
        ):
            executables.append(message["executable"])
    assert len(executables) == 1, executables
    shutil.copy2(executables[0], BINARY)
    listing = subprocess.check_output([str(BINARY), "--list"], text=True).splitlines()
    assert all(f"{PREFIX}{name}: test" in listing for name in NAMES), listing


if __name__ == "__main__":
    if sys.argv[1:] == ["build"]:
        build()
    else:
        assert len(sys.argv) == 3 and sys.argv[1] == "run"
        name = sys.argv[2]
        assert name in NAMES, name
        exact = PREFIX + name
        listing = subprocess.check_output(
            [str(BINARY), "--exact", exact, "--list"], text=True
        ).splitlines()
        assert listing.count(f"{exact}: test") == 1, listing
        sys.exit(subprocess.run(
            [str(BINARY), "--exact", exact, "--nocapture"]
        ).returncode)

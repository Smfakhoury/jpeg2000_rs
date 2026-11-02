import os
from pathlib import Path
import subprocess
import sys


CASES = {
    "metadata_identity": ["databin_view_metadata_identity_ignores_codestream"],
    "full_coverage": [
        "reassembles_out_of_order_fragments",
        "accepts_identical_duplicates_and_overlaps",
        "rejects_conflicting_overlap_without_mutation",
        "rejects_conflicting_final_lengths_and_bytes_past_final",
    ],
}

source = Path("src/jpip.rs").read_text()
assert "fn databin_view_metadata_identity_ignores_codestream()" in source
assert "        databin_view_full_coverage_sweep();" in source
assert "fn databin_view_full_coverage_sweep()" in source
binaries = [
    path for path in Path("target/debug/deps").glob("jpeg2000_rs-*")
    if path.is_file() and os.access(path, os.X_OK) and not path.suffix
]
assert len(binaries) == 1, f"Expected one built libtest binary, got {binaries}"
binary = str(binaries[0].resolve())
listing = subprocess.check_output([binary, "--list"], text=True).splitlines()
for case in CASES[sys.argv[1]]:
    name = f"jpip::tests::{case}"
    assert f"{name}: test" in listing, f"Missing test registration: {name}"
    print(f"Route: JpipFragment::new -> DatabinCache::insert -> databin_view; case={name}",
          flush=True)
    result = subprocess.run([binary, name, "--exact", "--nocapture", "--test-threads=1"])
    if result.returncode:
        sys.exit(result.returncode if result.returncode > 0 else 1)

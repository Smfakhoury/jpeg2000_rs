"""Install and verify the single Cargo-built libtest executable."""
from pathlib import Path
import shutil
import subprocess

source = Path("src/jpip.rs").read_text()
assert 'include!("jpip_databin_contract_tests.rs");' in source
names = [
    "jpip::tests::standard_databin_class_assignment",
    "jpip::tests::metadata_databin_identity_ignores_codestream",
    "jpip::tests::ordinary_extended_databin_identity_and_raw_class",
]
binaries = [
    path for path in Path("target/debug/deps").glob("jpeg2000_rs-*")
    if path.is_file() and path.stat().st_mode & 0o111 and not path.suffix
]
assert len(binaries) == 1, f"Expected exactly one library test executable: {binaries}"
listing = subprocess.check_output([str(binaries[0]), "--list"], text=True)
for name in names:
    assert f"{name}: test\n" in listing, f"Missing test registration: {name}"
shutil.copy2(binaries[0], "target/databin-contract-tests")

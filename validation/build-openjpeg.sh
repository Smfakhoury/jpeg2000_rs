#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output=${1:-"$root/target/validation/oracles"}
source_dir="$output/openjpeg-source"
build_dir="$output/openjpeg-build"
revision=a5891555eb49ed7cc26b2901ea680acda136d811

mkdir -p "$output"
if [ ! -e "$source_dir" ]; then
    git clone --no-checkout https://github.com/uclouvain/openjpeg.git "$source_dir"
    git -C "$source_dir" checkout --detach "$revision"
fi
if [ ! -d "$source_dir/.git" ] ||
    [ "$(git -C "$source_dir" rev-parse HEAD)" != "$revision" ] ||
    [ -n "$(git -C "$source_dir" status --porcelain)" ]; then
    echo "OpenJPEG source must be a clean checkout of $revision; use a fresh output directory" >&2
    exit 1
fi

cmake -S "$source_dir" -B "$build_dir" \
    -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
    -DBUILD_CODEC=ON -DBUILD_TESTING=OFF -DBUILD_JPIP=OFF \
    -DCMAKE_DISABLE_FIND_PACKAGE_PNG=TRUE -DCMAKE_DISABLE_FIND_PACKAGE_TIFF=TRUE \
    -DCMAKE_RUNTIME_OUTPUT_DIRECTORY="$build_dir/bin" \
    -DCMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE="$build_dir/bin"
cmake --build "$build_dir" --config Release --parallel 2 --target opj_compress opj_decompress

for program in opj_compress opj_decompress; do
    if [ ! -x "$build_dir/bin/$program" ] && [ ! -x "$build_dir/bin/$program.exe" ]; then
        echo "Missing OpenJPEG executable: $program" >&2
        exit 1
    fi
done
printf 'OpenJPEG source revision: %s\n' "$revision"

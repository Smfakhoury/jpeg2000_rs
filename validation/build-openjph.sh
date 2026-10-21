#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output=${1:-"$root/target/validation/oracles"}
source_dir="$output/openjph-source"
build_dir="$output/openjph-build"
revision=8ca3abdd3b185cd737e70c0d443171205b3e6f15

mkdir -p "$output"
if [ ! -e "$source_dir" ]; then
    git clone --no-checkout https://github.com/aous72/OpenJPH.git "$source_dir"
    git -C "$source_dir" checkout --detach "$revision"
fi
if [ ! -d "$source_dir/.git" ] ||
    [ "$(git -C "$source_dir" rev-parse HEAD)" != "$revision" ] ||
    [ -n "$(git -C "$source_dir" status --porcelain)" ]; then
    echo "OpenJPH source must be a clean checkout of $revision; use a fresh output directory" >&2
    exit 1
fi

cmake -S "$source_dir" -B "$build_dir" \
    -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
    -DOJPH_BUILD_TESTS=OFF -DOJPH_ENABLE_TIFF_SUPPORT=OFF -DOJPH_DISABLE_SIMD=ON
cmake --build "$build_dir" --config Release --parallel 2 --target ojph_compress ojph_expand

for program in ojph_compress ojph_expand; do
    directory="$build_dir/src/apps/$program"
    if [ ! -x "$directory/$program" ] && [ ! -x "$directory/$program.exe" ] &&
        [ ! -x "$directory/Release/$program.exe" ]; then
        echo "Missing OpenJPH executable: $program" >&2
        exit 1
    fi
done
printf 'OpenJPH source revision: %s\n' "$revision"

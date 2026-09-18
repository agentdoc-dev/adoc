#!/usr/bin/env bash
# Build the exact ONNX Runtime source required by ort-sys 2.0.0-rc.13.
set -euo pipefail

readonly ORT_COMMIT='da9b5e364c465de65c49d91e696cd6485270757f'
readonly ORT_REPOSITORY='https://github.com/microsoft/onnxruntime.git'

stage_dir=${1:?usage: build-intel-runtime.sh STAGE_DIR}
mkdir -p "$stage_dir"
stage_dir=$(cd "$stage_dir" && pwd)
work_dir=$(mktemp -d "${RUNNER_TEMP:-/tmp}/onnxruntime.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT

# The CPU-only shared library build uses Python's standard library.
command -v cmake >/dev/null || brew install cmake
git init --quiet "$work_dir/source"
git -C "$work_dir/source" remote add origin "$ORT_REPOSITORY"
git -C "$work_dir/source" fetch --depth=1 origin "$ORT_COMMIT"
git -C "$work_dir/source" checkout --quiet --detach FETCH_HEAD
test "$(git -C "$work_dir/source" rev-parse HEAD)" = "$ORT_COMMIT"

(
  cd "$work_dir/source"
  ./build.sh --config Release --build_shared_lib --skip_tests --parallel \
    --cmake_extra_defines CMAKE_OSX_ARCHITECTURES=x86_64
)

ort_lib_dir="$work_dir/source/build/MacOS/Release"
ort_basename="libonnxruntime.$(cat "$work_dir/source/VERSION_NUMBER").dylib"
ort_dylib="$ort_lib_dir/$ort_basename"
test -f "$ort_dylib"
install_name_tool -id "@rpath/$ort_basename" "$ort_dylib"
otool -D "$ort_dylib" | grep -Fx "@rpath/$ort_basename"
file "$ort_dylib" | grep -Eq 'Mach-O 64-bit dynamically linked shared library x86_64'

mkdir -p "$stage_dir/lib"
cp -a "$ort_lib_dir"/libonnxruntime*.dylib "$stage_dir/lib/"
cp "$work_dir/source/LICENSE" "$stage_dir/lib/LICENSE"
cp "$work_dir/source/ThirdPartyNotices.txt" "$stage_dir/lib/ThirdPartyNotices.txt"
find -L "$stage_dir/lib" -maxdepth 1 -name 'libonnxruntime*.dylib' -type f -print
test -e "$stage_dir/lib/$ort_basename"

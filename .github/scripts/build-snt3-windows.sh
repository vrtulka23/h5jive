#!/usr/bin/env bash

# Source this from the MSYS2 Windows build step so SNT3_BUILD_DIR reaches Cargo.
cargo fetch --locked --target x86_64-pc-windows-gnu
cargo_home=$(cygpath -u "${CARGO_HOME:-$USERPROFILE/.cargo}")
snt3_crate=$(find "$cargo_home/registry/src" -mindepth 2 -maxdepth 2 -type d -name 'scinumtools3-sys-0.10.2' -print -quit)
if [ -z "$snt3_crate" ]; then
  echo 'Bundled scinumtools3-sys source was not found' >&2
  exit 1
fi

# Patch a copy: changing Cargo's registry source can invalidate its checksum.
snt3_source="$PWD/target/snt3-windows-source"
mkdir -p "$snt3_source"
cp -R "$snt3_crate/vendor/snt3/." "$snt3_source/"
git apply --check --directory=target/snt3-windows-source .github/scripts/snt3-windows-tls.patch
git apply --directory=target/snt3-windows-source .github/scripts/snt3-windows-tls.patch

snt3_build="$PWD/target/snt3-windows-build"
snt3_build_native=$(cygpath -m "$snt3_build")
cmake -S "$(cygpath -m "$snt3_source")" -B "$snt3_build_native" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER="$(cygpath -m /ucrt64/bin/gcc.exe)" \
  -DCMAKE_CXX_COMPILER="$(cygpath -m /ucrt64/bin/g++.exe)" -DHDF5_ROOT="$(cygpath -m /ucrt64)" \
  -DENABLE_BINDING_C=ON -DENABLE_BINDING_PYTHON=OFF \
  -DENABLE_UNIT_TESTS=OFF -DENABLE_EXEC_APPS=OFF \
  -DENABLE_EXEC_EXAMPLES=OFF -DENABLE_EXEC_BENCHMARKS=OFF \
  -DENABLE_SNT_REPORT=OFF -DENABLE_MAT=OFF -DENABLE_API=OFF
cmake --build "$snt3_build_native" --config Release --target snt-c --parallel "$(nproc)"
export SNT3_BUILD_DIR="$snt3_build_native"
export SNT3_HDF5_LIB_DIR="$(cygpath -m /ucrt64/lib)"

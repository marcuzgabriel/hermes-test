#!/usr/bin/env bash
set -euo pipefail

HERMES_REPO="${HERMES_REPO:-/work/hermes-test}"
TOPDANMARK_REPO="${TOPDANMARK_REPO:-/work/mobile-insurance-app-expo}"
TOPDANMARK_APP_PATH="${TOPDANMARK_APP_PATH:-apps/topdanmark}"
HERMES_COMMIT="${HERMES_COMMIT:-fd0e1d3ed}"
HERMES_BUILD_JOBS="${HERMES_BUILD_JOBS:-4}"
HERMES_BUILD_DIR="${HERMES_BUILD_DIR:-build}"
SKIP_HERMES_INSTALL="${SKIP_HERMES_INSTALL:-0}"
SKIP_TOPDANMARK_INSTALL="${SKIP_TOPDANMARK_INSTALL:-${SKIP_INSTALL:-0}}"
TOPDANMARK_IGNORE_SCRIPTS="${TOPDANMARK_IGNORE_SCRIPTS:-1}"

if [[ ! -f "${HERMES_REPO}/Cargo.toml" ]]; then
  echo "Missing hermes-test repo at ${HERMES_REPO}" >&2
  exit 1
fi

if [[ ! -f "${TOPDANMARK_REPO}/package.json" ]]; then
  echo "Missing mobile-insurance-app-expo repo at ${TOPDANMARK_REPO}" >&2
  exit 1
fi

if [[ ! -d "${HERMES_REPO}/vendor/hermes/.git" ]]; then
  echo "vendor/hermes not found, cloning Hermes..."
  for i in 1 2 3; do
    if git clone --filter=blob:none https://github.com/facebook/hermes.git "${HERMES_REPO}/vendor/hermes"; then
      break
    fi
    echo "Clone attempt ${i} failed, retrying..."
    rm -rf "${HERMES_REPO}/vendor/hermes"
    sleep 5
  done
fi

pushd "${HERMES_REPO}/vendor/hermes" >/dev/null
git fetch --depth=1 origin || true
git checkout "${HERMES_COMMIT}"
if grep -q 'FATAL_ERROR "Host compiler' external/llvh/cmake/modules/CheckAtomic.cmake; then
  sed -i 's/FATAL_ERROR "Host compiler/WARNING "Host compiler/g' external/llvh/cmake/modules/CheckAtomic.cmake
fi
if ! grep -q '^#include <cstdint>$' lib/Platform/Intl/impl_icu/IntlUtils.cpp; then
  sed -i '1i #include <cstdint>' lib/Platform/Intl/impl_icu/IntlUtils.cpp
fi

# build.rs links against vendor/hermes/build, so default to that path.
# If this folder was created on host macOS, wipe incompatible CMake cache.
if [[ -f "${HERMES_BUILD_DIR}/CMakeCache.txt" ]] && ! grep -q '^CMAKE_HOME_DIRECTORY:INTERNAL=/work/hermes-test/vendor/hermes$' "${HERMES_BUILD_DIR}/CMakeCache.txt"; then
  rm -rf "${HERMES_BUILD_DIR}"
fi

cmake -S . -B "${HERMES_BUILD_DIR}" -G Ninja \
  -DHERMES_STATIC_LINK=OFF -DBUILD_SHARED_LIBS=OFF \
  -DHERMES_BUILD_APPLE_FRAMEWORK=OFF -DHERMES_ENABLE_DEBUGGER=OFF \
  -DHERMES_ENABLE_INTL=ON \
  -DHERMES_ENABLE_TEST_SUITE=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build "${HERMES_BUILD_DIR}" --parallel "${HERMES_BUILD_JOBS}"

# Keep parity with Release workflow: some Linux builds require this header artifact.
mkdir -p "${HERMES_BUILD_DIR}/external/llvh/include/llvh/IR"
if [[ ! -f "${HERMES_BUILD_DIR}/external/llvh/include/llvh/IR/Attributes.inc" ]]; then
  cat > "${HERMES_BUILD_DIR}/external/llvh/include/llvh/IR/Attributes.inc" <<'EOF'
Alignment,
AllocSize,
AlwaysInline,
ArgMemOnly,
Builtin,
ByVal,
Cold,
Convergent,
Dereferenceable,
DereferenceableOrNull,
InaccessibleMemOnly,
InaccessibleMemOrArgMemOnly,
InAlloca,
InlineHint,
InReg,
JumpTable,
MinSize,
Naked,
Nest,
NoAlias,
NoBuiltin,
NoCapture,
NoCfCheck,
NoDuplicate,
NoFree,
NoImplicitFloat,
NoInline,
NoRecurse,
NoReturn,
NoSync,
NoUnwind,
NonLazyBind,
NonNull,
OptForFuzzing,
OptimizeForSize,
OptimizeNone,
ReadNone,
ReadOnly,
Returned,
ReturnsTwice,
SExt,
SafeStack,
SanitizeAddress,
SanitizeHWAddress,
SanitizeMemory,
SanitizeThread,
ShadowCallStack,
Speculatable,
SpeculativeLoadHardening,
StackAlignment,
StackProtect,
StackProtectReq,
StackProtectStrong,
StrictFP,
StructRet,
SwiftError,
SwiftSelf,
UWTable,
WillReturn,
WriteOnly,
ZExt,
EOF
fi
popd >/dev/null

pushd "${HERMES_REPO}/packages/hermes-test" >/dev/null
cp package.json package.json.bak
cleanup_pkg_json() {
  if [[ -f package.json.bak ]]; then
    mv package.json.bak package.json
  fi
}
trap cleanup_pkg_json EXIT
node -e "const fs=require('fs');const p=require('./package.json');delete p.optionalDependencies;fs.writeFileSync('./package.json', JSON.stringify(p,null,2)+'\n')"
if [[ "${SKIP_HERMES_INSTALL}" != "1" ]]; then
  bun install --no-save
fi
node bundle.mjs
cleanup_pkg_json
trap - EXIT
popd >/dev/null

pushd "${HERMES_REPO}" >/dev/null
cargo build --release
popd >/dev/null

pushd "${TOPDANMARK_REPO}" >/dev/null
if [[ "${SKIP_TOPDANMARK_INSTALL}" != "1" ]]; then
  if [[ "${TOPDANMARK_IGNORE_SCRIPTS}" == "1" ]]; then
    bun install --no-save --ignore-scripts
  else
    bun install --no-save
  fi
fi
popd >/dev/null

pushd "${TOPDANMARK_REPO}/${TOPDANMARK_APP_PATH}" >/dev/null
TZ="${TZ:-Europe/Copenhagen}" "${HERMES_REPO}/target/release/hermes-test" "$@"
popd >/dev/null

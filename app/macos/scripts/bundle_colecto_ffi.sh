#!/bin/sh
# Bundle libcolecto_ffi.dylib into the macOS app Frameworks directory when built.
set -eu

REPO_ROOT="${PROJECT_DIR}/../.."
PROFILE="debug"
if [ "${CONFIGURATION:-Debug}" != "Debug" ]; then
  PROFILE="release"
fi

SRC=""
if [ -n "${COLECTO_FFI_LIB:-}" ] && [ -f "${COLECTO_FFI_LIB}" ]; then
  SRC="${COLECTO_FFI_LIB}"
elif [ -f "${REPO_ROOT}/target/${PROFILE}/libcolecto_ffi.dylib" ]; then
  SRC="${REPO_ROOT}/target/${PROFILE}/libcolecto_ffi.dylib"
elif [ -f "${REPO_ROOT}/target/release/libcolecto_ffi.dylib" ]; then
  SRC="${REPO_ROOT}/target/release/libcolecto_ffi.dylib"
elif [ -f "${REPO_ROOT}/target/debug/libcolecto_ffi.dylib" ]; then
  SRC="${REPO_ROOT}/target/debug/libcolecto_ffi.dylib"
fi

if [ -n "${SRC}" ]; then
  DEST_DIR="${BUILT_PRODUCTS_DIR}/${FRAMEWORKS_FOLDER_PATH}"
  mkdir -p "${DEST_DIR}"
  cp -f "${SRC}" "${DEST_DIR}/libcolecto_ffi.dylib"
  install_name_tool -id "@rpath/libcolecto_ffi.dylib" "${DEST_DIR}/libcolecto_ffi.dylib" 2>/dev/null || true
  if [ -n "${EXPANDED_CODE_SIGN_IDENTITY:-}" ] && [ "${EXPANDED_CODE_SIGN_IDENTITY}" != "-" ]; then
    codesign --force --sign "${EXPANDED_CODE_SIGN_IDENTITY}" --timestamp=none "${DEST_DIR}/libcolecto_ffi.dylib"
  fi
fi

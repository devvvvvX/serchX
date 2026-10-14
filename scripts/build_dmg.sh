#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# serchX Optimized Release DMG Generator
# Builds an optimized release bundle and packages a distributable macOS .dmg
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

# Dynamic version and architecture discovery
VERSION="$(grep '^version =' src-tauri/Cargo.toml | head -n1 | cut -d '"' -f2)"
ARCH="$(uname -m)"
if [ "$ARCH" = "arm64" ]; then
  ARCH_TAG="aarch64"
else
  ARCH_TAG="$ARCH"
fi

DMG_NAME="serchX_${VERSION}_${ARCH_TAG}.dmg"
DMG_DIR="${REPO_ROOT}/src-tauri/target/release/bundle/dmg"
DMG_PATH="${DMG_DIR}/${DMG_NAME}"
APP_PATH="${REPO_ROOT}/src-tauri/target/release/bundle/macos/serchX.app"

# Ensure clang runtime is available for linker if needed
CLANG_RT="$(clang -print-file-name=libclang_rt.osx.a 2>/dev/null || true)"
if [ -n "$CLANG_RT" ] && [ -f "$CLANG_RT" ]; then
  export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=${CLANG_RT}"
fi

echo "==> Building optimized serchX release application (v${VERSION}, ${ARCH_TAG})..."
cargo tauri build --bundles app

if [ ! -d "${APP_PATH}" ]; then
  echo "ERROR: Expected app bundle not found at ${APP_PATH}" >&2
  exit 1
fi

echo "==> Verifying and ensuring ad-hoc code signature..."
codesign --force --deep -s - "${APP_PATH}"
codesign --verify --deep --strict "${APP_PATH}"

echo "==> Preparing DMG staging environment with /Applications shortcut..."
STAGING_DIR="$(mktemp -d /tmp/serchx_dmg_staging.XXXXXX)"
cleanup() {
  rm -rf "${STAGING_DIR}"
}
trap cleanup EXIT

# Populate staging directory
cp -R "${APP_PATH}" "${STAGING_DIR}/serchX.app"
ln -s /Applications "${STAGING_DIR}/Applications"

echo "==> Creating optimized release DMG installer (maximum compression)..."
mkdir -p "${DMG_DIR}"
rm -f "${DMG_PATH}"

hdiutil create \
  -volname "serchX" \
  -srcfolder "${STAGING_DIR}" \
  -ov \
  -format UDZO \
  -imagekey zlib-level=9 \
  "${DMG_PATH}"

echo "==> Verifying DMG mounting and filesystem structure..."
MOUNT_DIR="/Volumes/serchX-Verify"
if [ -d "${MOUNT_DIR}" ]; then
  hdiutil detach "${MOUNT_DIR}" -quiet || true
fi

hdiutil attach "${DMG_PATH}" -mountpoint "${MOUNT_DIR}" -quiet
test -d "${MOUNT_DIR}/serchX.app"
test -L "${MOUNT_DIR}/Applications"
hdiutil detach "${MOUNT_DIR}" -quiet

echo "==> Generating SHA-256 checksum..."
cd "${DMG_DIR}"
shasum -a 256 "${DMG_NAME}" > "${DMG_NAME}.sha256"
cd "${REPO_ROOT}"

DMG_SIZE="$(ls -lh "${DMG_PATH}" | awk '{print $5}')"
SHA256="$(awk '{print $1}' "${DMG_DIR}/${DMG_NAME}.sha256")"

echo ""
echo "=========================================================="
echo "  serchX Release DMG Generated Successfully!"
echo "=========================================================="
echo "  Artifact:   ${DMG_PATH}"
echo "  File Size:  ${DMG_SIZE}"
echo "  Version:    ${VERSION} (${ARCH_TAG})"
echo "  SHA-256:    ${SHA256}"
echo "  Checksum:   ${DMG_DIR}/${DMG_NAME}.sha256"
echo "=========================================================="

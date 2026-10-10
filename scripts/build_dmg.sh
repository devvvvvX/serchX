#!/usr/bin/env bash
set -e

# Build the release bundle and create distributable macOS .dmg
echo "==> Building serchX release application..."
cargo build --release --manifest-path src-tauri/Cargo.toml

echo "==> Packaging application into serchX.app..."
cargo tauri build --bundles app

echo "==> Creating distributable DMG installer..."
mkdir -p src-tauri/target/release/bundle/dmg
hdiutil create -volname "serchX" \
  -srcfolder "src-tauri/target/release/bundle/macos/serchX.app" \
  -ov -format UDZO \
  "src-tauri/target/release/bundle/dmg/serchX_0.1.0_aarch64.dmg"

echo "==> Verifying DMG mounting..."
hdiutil attach src-tauri/target/release/bundle/dmg/serchX_0.1.0_aarch64.dmg -mountpoint /Volumes/serchX-Verify -quiet
test -d "/Volumes/serchX-Verify/serchX.app"
hdiutil detach /Volumes/serchX-Verify -quiet

echo "==> DMG verified successfully: src-tauri/target/release/bundle/dmg/serchX_0.1.0_aarch64.dmg"

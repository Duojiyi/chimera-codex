#!/usr/bin/env bash
set -euo pipefail

# Chimera Codex macOS DMG packager.
# Codesign is ad-hoc only (--sign -). There is no Developer ID signing and no notarization.

VERSION="${1:-0.0.0}"
ARCH="${2:-$(uname -m)}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
DIST="$ROOT/dist/macos"
STAGE="$DIST/stage"
BINARY_DIR="${BINARY_DIR:-$ROOT/target/release}"
DMG="$DIST/ChimeraCodex-${VERSION}-macos-${ARCH}.dmg"
ICON_SOURCE="$ROOT/apps/codex-plus-manager/src-tauri/icons/icon.png"
ICON_NAME="codex-plus-plus.icns"
ICON_ICNS="$DIST/$ICON_NAME"

# CFBundleShortVersionString must be pure X.Y.Z (strip -chimera.N)
SHORT_VERSION="${VERSION%%-*}"
# CFBundleVersion must be a strictly increasing integer from brand/product.toml
if [ -z "${MACOS_BUILD_NUMBER:-}" ]; then
  MACOS_BUILD_NUMBER="$(
    awk -F'=' '/^macos_build_number[[:space:]]*=/ {
      gsub(/[[:space:]]/, "", $2);
      print $2;
      exit
    }' "$ROOT/brand/product.toml"
  )"
fi
if ! [[ "$MACOS_BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: macos_build_number must be a positive integer, got: ${MACOS_BUILD_NUMBER:-<empty>}" >&2
  exit 1
fi

SILENT_APP_NAME="Chimera Codex"
MANAGER_APP_NAME="Chimera Codex 管理工具"

rm -rf "$DIST"
mkdir -p "$STAGE"

prepare_icon() {
  local iconset="$DIST/codex-plus-plus.iconset"
  rm -rf "$iconset"
  mkdir -p "$iconset"

  sips -z 16 16 "$ICON_SOURCE" --out "$iconset/icon_16x16.png" >/dev/null
  sips -z 32 32 "$ICON_SOURCE" --out "$iconset/icon_16x16@2x.png" >/dev/null
  sips -z 32 32 "$ICON_SOURCE" --out "$iconset/icon_32x32.png" >/dev/null
  sips -z 64 64 "$ICON_SOURCE" --out "$iconset/icon_32x32@2x.png" >/dev/null
  sips -z 128 128 "$ICON_SOURCE" --out "$iconset/icon_128x128.png" >/dev/null
  sips -z 256 256 "$ICON_SOURCE" --out "$iconset/icon_128x128@2x.png" >/dev/null
  sips -z 256 256 "$ICON_SOURCE" --out "$iconset/icon_256x256.png" >/dev/null
  sips -z 512 512 "$ICON_SOURCE" --out "$iconset/icon_256x256@2x.png" >/dev/null
  sips -z 512 512 "$ICON_SOURCE" --out "$iconset/icon_512x512.png" >/dev/null
  sips -z 1024 1024 "$ICON_SOURCE" --out "$iconset/icon_512x512@2x.png" >/dev/null

  iconutil -c icns "$iconset" -o "$ICON_ICNS"
}

create_app() {
  local app_name="$1"
  local executable_name="$2"
  local binary_path="$3"
  local bundle_id="$4"
  local lsui_element="${5:-false}"
  local app_dir="$STAGE/$app_name.app"

  if [ ! -x "$binary_path" ]; then
    echo "error: binary not found or not executable: $binary_path" >&2
    return 1
  fi

  rm -rf "$app_dir"
  mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
  cp "$binary_path" "$app_dir/Contents/MacOS/$executable_name"
  cp "$ICON_ICNS" "$app_dir/Contents/Resources/$ICON_NAME"
  chmod +x "$app_dir/Contents/MacOS/$executable_name"
  printf 'APPL????' > "$app_dir/Contents/PkgInfo"
  cat > "$app_dir/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>$app_name</string>
  <key>CFBundleDisplayName</key>
  <string>$app_name</string>
  <key>CFBundleIdentifier</key>
  <string>$bundle_id</string>
  <key>CFBundleVersion</key>
  <string>$MACOS_BUILD_NUMBER</string>
  <key>CFBundleShortVersionString</key>
  <string>$SHORT_VERSION</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleSignature</key>
  <string>????</string>
  <key>CFBundleExecutable</key>
  <string>$executable_name</string>
  <key>CFBundleIconFile</key>
  <string>$ICON_NAME</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>LSUIElement</key>
  <$lsui_element/>
</dict>
</plist>
PLIST
}

sign_app() {
  # Ad-hoc only: validates bundle structure. No Developer ID, no notarization.
  local app_dir="$1"
  local executable
  executable="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app_dir/Contents/Info.plist")"
  codesign --force --sign - "$app_dir/Contents/MacOS/$executable"
  codesign --force --sign - "$app_dir"
}

verify_app() {
  local app_dir="$1"
  local plist="$app_dir/Contents/Info.plist"
  local plutil_bin
  plutil_bin="$(command -v plutil || true)"
  if [ -n "$plutil_bin" ]; then
    "$plutil_bin" -lint "$plist" >/dev/null
  else
    /usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist" >/dev/null
  fi
  if [ ! -f "$app_dir/Contents/PkgInfo" ]; then
    echo "error: missing PkgInfo in $app_dir" >&2
    return 1
  fi
  local short_ver build_ver
  short_ver="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")"
  build_ver="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$plist")"
  if [[ "$short_ver" == *"-"* ]]; then
    echo "error: CFBundleShortVersionString must be numeric X.Y.Z, got: $short_ver" >&2
    return 1
  fi
  if ! [[ "$build_ver" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: CFBundleVersion must be a positive integer, got: $build_ver" >&2
    return 1
  fi
  codesign -dv "$app_dir" >/dev/null 2>&1 || {
    echo "error: codesign verification failed for $app_dir" >&2
    return 1
  }
}

write_migration_readme() {
  cat > "$STAGE/README.txt" <<EOF
Chimera Codex — macOS install notes
===================================

This DMG is ad-hoc signed only. It is NOT Developer ID signed and NOT notarized.
If Gatekeeper blocks launch: right-click the app → Open, or clear quarantine.

Upgrade from Codex++ (legacy):
1. Quit Codex++ / Codex++ 管理工具 completely.
2. Drag "Chimera Codex.app" and "Chimera Codex 管理工具.app" into Applications.
3. Manually delete old "Codex++.app" and "Codex++ 管理工具.app".
   Chimera does not delete legacy apps automatically.
4. Open Applications and launch the new Chimera apps (right-click → Open if needed).

Architecture: ${ARCH}
Version: ${VERSION} (CFBundleShortVersionString=${SHORT_VERSION}, CFBundleVersion=${MACOS_BUILD_NUMBER})
EOF
}

prepare_icon
# Bundle ID and executable names stay compatible with upstream for phase 1.
create_app "$SILENT_APP_NAME" "CodexPlusPlus" "$BINARY_DIR/codex-plus-plus" "com.bigpizzav3.codexplusplus" "true"
create_app "$MANAGER_APP_NAME" "CodexPlusPlusManager" "$BINARY_DIR/codex-plus-plus-manager" "com.bigpizzav3.codexplusplus.manager" "false"

sign_app "$STAGE/$SILENT_APP_NAME.app"
sign_app "$STAGE/$MANAGER_APP_NAME.app"

verify_app "$STAGE/$SILENT_APP_NAME.app"
verify_app "$STAGE/$MANAGER_APP_NAME.app"

write_migration_readme
ln -s /Applications "$STAGE/Applications"

hdiutil create -volname "Chimera Codex" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
echo "$DMG"

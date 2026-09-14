#!/bin/sh
# Build a release binary and wrap it in ReWorkspace.app (+ .dmg). Run from the repo root on macOS.
set -eu
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(uname -m); [ "$ARCH" = "arm64" ] || ARCH=x64
cargo build --release
APP=dist/ReWorkspace.app
rm -rf dist && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" dist/icon.iconset
cp target/release/reworkspace "$APP/Contents/MacOS/ReWorkspace"
cp THIRD_PARTY_LICENSES.md assets/fonts/LICENSE-Liberation.txt "$APP/Contents/Resources/"
# app icon from our own workspace.svg
for px in 16 32 64 128 256 512 1024; do
  target/release/reworkspace --render-icon "dist/icon.iconset/icon_${px}x${px}.png" --px "$px"
done
for px in 16 32 128 256 512; do cp "dist/icon.iconset/icon_$((px*2))x$((px*2)).png" "dist/icon.iconset/icon_${px}x${px}@2x.png"; done
rm dist/icon.iconset/icon_64x64.png dist/icon.iconset/icon_1024x1024.png
iconutil -c icns dist/icon.iconset -o "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>ReWorkspace</string>
  <key>CFBundleDisplayName</key><string>ReWorkspace</string>
  <key>CFBundleIdentifier</key><string>com.reworkspace.desktop</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>ReWorkspace</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
</dict></plist>
PLIST
rm -rf dist/icon.iconset
codesign --force --sign - "$APP" 2>/dev/null || true   # ad-hoc signature so the binary runs at all on Apple Silicon
hdiutil create -volname ReWorkspace -srcfolder "$APP" -ov -format UDZO "dist/ReWorkspace-$VERSION-macos-$ARCH.dmg" >/dev/null
du -sh "$APP" dist/*.dmg

#!/usr/bin/env bash
# Builds GlideDeck.app and a drag-to-install GlideDeck-<version>.dmg on macOS.
#
#   packaging/macos/build-dmg.sh              # build for this Mac's architecture
#   UNIVERSAL=1 packaging/macos/build-dmg.sh  # Apple Silicon + Intel universal binary
#
# Optional signing / notarization (see README → "Creating a macOS DMG"):
#   SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
#   NOTARY_PROFILE="glidedeck-notary"   # created with `xcrun notarytool store-credentials`
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo"
version="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n1)"
dist="$repo/dist"
app="$dist/GlideDeck.app"
mkdir -p "$dist"

echo "==> Building GlideDeck $version"
if [[ "${UNIVERSAL:-0}" == "1" ]]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  cargo build --release -p glidedeck --target aarch64-apple-darwin
  cargo build --release -p glidedeck --target x86_64-apple-darwin
  bin="$repo/target/universal/glidedeck"
  mkdir -p "$(dirname "$bin")"
  lipo -create -output "$bin" \
    target/aarch64-apple-darwin/release/glidedeck \
    target/x86_64-apple-darwin/release/glidedeck
else
  cargo build --release -p glidedeck
  bin="$repo/target/release/glidedeck"
fi

echo "==> Assembling $app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/GlideDeck"
sed "s/__VERSION__/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
cp examples/demo.glide.json "$app/Contents/Resources/"

echo "==> Generating the app icon"
iconset="$dist/GlideDeck.iconset"
rm -rf "$iconset" && mkdir -p "$iconset"
for s in 16 32 128 256 512; do
  cp "assets/icon/glidedeck-$s.png" "$iconset/icon_${s}x${s}.png"
  cp "assets/icon/glidedeck-$((s * 2)).png" "$iconset/icon_${s}x${s}@2x.png" 2>/dev/null \
    || sips -z $((s * 2)) $((s * 2)) "assets/icon/glidedeck-1024.png" --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/GlideDeck.icns"
rm -rf "$iconset"

if [[ -n "${SIGN_IDENTITY:-}" ]]; then
  echo "==> Signing with: $SIGN_IDENTITY"
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$app"
else
  echo "==> Ad-hoc signing (fine for your own Macs; use SIGN_IDENTITY to distribute)"
  codesign --force --sign - "$app"
fi

echo "==> Creating the DMG"
dmg="$dist/GlideDeck-$version.dmg"
staging="$dist/dmg-staging"
rm -rf "$staging" "$dmg" && mkdir -p "$staging"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
hdiutil create -volname "GlideDeck $version" -srcfolder "$staging" -ov -format UDZO "$dmg"
rm -rf "$staging"

if [[ -n "${SIGN_IDENTITY:-}" ]]; then
  codesign --force --timestamp --sign "$SIGN_IDENTITY" "$dmg"
fi
if [[ -n "${NOTARY_PROFILE:-}" ]]; then
  echo "==> Notarizing (this can take a few minutes)"
  xcrun notarytool submit "$dmg" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$dmg"
fi

echo "==> Done: $dmg"

#!/usr/bin/env bash
# Build Bloxide.app for the iOS simulator or a device.
#
#   ios/build.sh sim          build for the simulator
#   ios/build.sh sim run      build, then boot an iPhone simulator and launch
#                             there (BLOXIDE_SIMULATOR picks the device name,
#                             default "iPhone 17 Pro"); any further arguments
#                             go to the app, e.g. --layout=3
#   ios/build.sh device       build and sign for a device; needs
#                             BLOXIDE_SIGN_IDENTITY ("Apple Development: ...")
#                             and BLOXIDE_PROFILE (path to a .mobileprovision)
#
# BLOXIDE_BUNDLE_ID overrides the bundle identifier (default dev.bloxide.game);
# a device build's identifier must match its provisioning profile.
set -euo pipefail

cd "$(dirname "$0")/.."

kind="${1:-sim}"
action="${2:-}"
shift $(($# < 2 ? $# : 2))
bundle_id="${BLOXIDE_BUNDLE_ID:-dev.bloxide.game}"
simulator="${BLOXIDE_SIMULATOR:-iPhone 17 Pro}"

case "$kind" in
  sim) target=aarch64-apple-ios-sim ;;
  device) target=aarch64-apple-ios ;;
  *)
    echo "usage: ios/build.sh sim|device [run]" >&2
    exit 2
    ;;
esac

rustup target add "$target" >/dev/null
cargo build --release --target "$target"

app="target/ios/$kind/Bloxide.app"
rm -rf "$app"
mkdir -p "$app"
cp "target/$target/release/bloxide" "$app/bloxide"
sed "s/\$(BUNDLE_ID)/$bundle_id/" ios/Info.plist > "$app/Info.plist"

if [ "$kind" = sim ]; then
  codesign --force --sign - "$app"
else
  : "${BLOXIDE_SIGN_IDENTITY:?set BLOXIDE_SIGN_IDENTITY to your signing identity}"
  : "${BLOXIDE_PROFILE:?set BLOXIDE_PROFILE to a .mobileprovision path}"
  cp "$BLOXIDE_PROFILE" "$app/embedded.mobileprovision"
  entitlements="$(mktemp)"
  security cms -D -i "$BLOXIDE_PROFILE" > "$entitlements.plist"
  /usr/libexec/PlistBuddy -x -c "Print :Entitlements" "$entitlements.plist" > "$entitlements"
  codesign --force --sign "$BLOXIDE_SIGN_IDENTITY" --entitlements "$entitlements" "$app"
  rm -f "$entitlements" "$entitlements.plist"
fi

echo "built $app"

if [ "$action" = run ]; then
  if [ "$kind" != sim ]; then
    echo "run is only supported for the simulator; install device builds with Xcode or devicectl" >&2
    exit 2
  fi
  xcrun simctl boot "$simulator" 2>/dev/null || true
  open -a Simulator
  xcrun simctl terminate "$simulator" "$bundle_id" 2>/dev/null || true
  xcrun simctl install "$simulator" "$app"
  xcrun simctl launch "$simulator" "$bundle_id" ${@+"$@"}
fi

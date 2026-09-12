#!/usr/bin/env bash
# Drives the debug APK on a running emulator and screenshots each screen.
#
# This is the Android equivalent of the browser smoke run on the web client: the build
# passing proves the code compiles, not that a fitter can see anything. Run it against a
# booted emulator with the backend running on the host.
#
#   ANDROID_HOME=… JAVA_HOME=… ./scripts/smoke.sh out-dir
set -euo pipefail

OUT="${1:-smoke-shots}"
ADB="${ANDROID_HOME:?set ANDROID_HOME}/platform-tools/adb"
PKG=hu.autotherm.autocrm.debug
ACTIVITY="$PKG/hu.autotherm.autocrm.MainActivity"

mkdir -p "$OUT"

shot() {
  sleep "${2:-2}"
  "$ADB" exec-out screencap -p > "$OUT/$1.png"
  echo "  $1.png"
}

tap()  { "$ADB" shell input tap "$1" "$2"; sleep 1; }
type() { "$ADB" shell input text "$1"; }

echo "Installing…"
"$ADB" install -r -g ../app/build/outputs/apk/debug/app-debug.apk >/dev/null
# -g grants the manifest permissions up front: the camera dialog is Android's, not ours,
# and tapping through it adds nothing to what this run is checking.

echo "Starting…"
"$ADB" shell am force-stop "$PKG"
"$ADB" shell pm clear "$PKG" >/dev/null
"$ADB" shell am start -n "$ACTIVITY" >/dev/null
shot 01-login 4

echo "Signing in…"
tap 540 700            # e-mail field
type "${E2E_EMAIL:-e2e@autotherm.hu}"
tap 540 900            # password field
type "${E2E_PASSWORD:-e2e-smoke-password-1234}"
"$ADB" shell input keyevent 111   # ESC closes the keyboard
tap 540 1100           # sign in
shot 02-capture-no-order 5

echo "Picking an order…"
tap 540 190            # the header opens the picker
shot 03-picker 3
tap 540 500            # first result
shot 04-capture-ready 3

echo "Other tabs…"
tap 400 2180 ; shot 05-orders 3
tap 700 2180 ; shot 06-people 3
tap 950 2180 ; shot 07-queue 3

echo "Done. Screenshots in $OUT/"

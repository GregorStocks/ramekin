#!/usr/bin/env bash
# Boot and configure a simulator for the iOS UI tests on CI.
#
# Picks the newest base-model iPhone (lighter than a Pro Max for a small CI
# runner), boots it, waits until booting has fully finished, and turns off
# keyboard extras (autocorrect, predictions, slide-to-type and its first-use
# tutorials) that cost time the first time a test focuses a text field.
#
# Prints GITHUB_ENV lines on stdout; progress goes to stderr:
#   IOS_UI_UDID=<udid>
#   IOS_UI_DESTINATION=platform=iOS Simulator,id=<udid>
set -euo pipefail

cd "$(dirname "$0")/.."

DEVICES=$(xcrun simctl list devices available -j)
UDID=$(python3 scripts/find-ios-simulator.py --base-model <<<"$DEVICES")
NAME=$(python3 -c '
import json, sys
udid = sys.argv[1]
for devices in json.load(sys.stdin)["devices"].values():
    for d in devices:
        if d["udid"] == udid:
            print(d["name"])
' "$UDID" <<<"$DEVICES")
echo "Preparing $NAME ($UDID)" >&2

# -b boots the device if needed and returns once it has finished booting,
# including first-boot data migration.
xcrun simctl bootstatus "$UDID" -b >&2

for key in KeyboardAutocorrection KeyboardPrediction KeyboardShowPredictionBar \
    KeyboardContinuousPathEnabled; do
    xcrun simctl spawn "$UDID" defaults write com.apple.Preferences "$key" -bool NO
done
for key in DidShowContinuousPathIntroduction DidShowGestureKeyboardIntroduction \
    KeyboardDidShowProductivityTutorial; do
    xcrun simctl spawn "$UDID" defaults write com.apple.Preferences "$key" -bool YES
done

echo "IOS_UI_UDID=$UDID"
echo "IOS_UI_DESTINATION=platform=iOS Simulator,id=$UDID"

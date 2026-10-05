#!/usr/bin/env bash
# Boot and configure a simulator for the iOS UI tests on CI.
#
# A freshly booted iOS 26 simulator spends minutes saturating a small CI
# runner: data migration, then home-screen widget extensions and system
# daemons (load averages above 400 on a 3-core runner). Tests that start
# during that storm stall, and xcodebuild can even fail to find the device.
# So CI boots early and configures late:
#
#   boot       Pick the newest base-model iPhone (lighter than a Pro Max) and
#              start booting it without waiting. Run this at the start of the
#              job so the boot storm overlaps the Rust builds.
#   configure  Wait for the boot to finish, turn off keyboard extras
#              (autocorrect, predictions, slide-to-type and its first-use
#              tutorials), then wait for the runner's load to settle.
#              Needs IOS_UI_UDID from the boot phase.
#
# The boot phase prints GITHUB_ENV lines on stdout; progress goes to stderr:
#   IOS_UI_UDID=<udid>
#   IOS_UI_DESTINATION=platform=iOS Simulator,id=<udid>
set -euo pipefail

cd "$(dirname "$0")/.."

# Proceed once the 1-minute load average is below this many runnable threads
# per CPU, or after SETTLE_TIMEOUT seconds regardless.
SETTLE_LOAD_PER_CPU=2
SETTLE_TIMEOUT=300

boot() {
    local devices udid name
    devices=$(xcrun simctl list devices available -j)
    udid=$(python3 scripts/find-ios-simulator.py --base-model <<<"$devices")
    name=$(python3 -c '
import json, sys
udid = sys.argv[1]
for devices in json.load(sys.stdin)["devices"].values():
    for d in devices:
        if d["udid"] == udid:
            print(d["name"])
' "$udid" <<<"$devices")
    echo "Booting $name ($udid)" >&2
    xcrun simctl boot "$udid" >&2
    echo "IOS_UI_UDID=$udid"
    echo "IOS_UI_DESTINATION=platform=iOS Simulator,id=$udid"
}

load_average() {
    sysctl -n vm.loadavg | awk '{print $2}'
}

configure() {
    local udid=${IOS_UI_UDID:?run the boot phase first}
    # Returns once booting, including first-boot data migration, finished.
    xcrun simctl bootstatus "$udid" -b >&2

    local key
    for key in KeyboardAutocorrection KeyboardPrediction KeyboardShowPredictionBar \
        KeyboardContinuousPathEnabled; do
        xcrun simctl spawn "$udid" defaults write com.apple.Preferences "$key" -bool NO
    done
    for key in DidShowContinuousPathIntroduction DidShowGestureKeyboardIntroduction \
        KeyboardDidShowProductivityTutorial; do
        xcrun simctl spawn "$udid" defaults write com.apple.Preferences "$key" -bool YES
    done

    local limit start load
    limit=$(($(sysctl -n hw.ncpu) * SETTLE_LOAD_PER_CPU))
    start=$SECONDS
    while true; do
        load=$(load_average)
        echo "Load average $load (waiting for < $limit)" >&2
        if awk -v l="$load" -v m="$limit" 'BEGIN { exit !(l < m) }'; then
            break
        fi
        if ((SECONDS - start >= SETTLE_TIMEOUT)); then
            echo "::warning::Runner load still $load after ${SETTLE_TIMEOUT}s; starting UI tests anyway"
            break
        fi
        sleep 10
    done
}

case "${1:-}" in
    boot) boot ;;
    configure) configure ;;
    *)
        echo "usage: $0 boot|configure" >&2
        exit 2
        ;;
esac

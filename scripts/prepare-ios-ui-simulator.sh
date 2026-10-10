#!/usr/bin/env bash
# Boot and configure a simulator for the iOS UI tests on CI.
#
# A freshly booted iOS 26 simulator spends minutes saturating a small CI
# runner: data migration, then home-screen widget extensions and system
# daemons. Tests that start during that storm stall, and xcodebuild can even
# fail to find the device. So CI boots early and configures late:
#
#   boot       Pick the newest base-model iPhone (lighter than a Pro Max) and
#              start booting it without waiting. Run this at the start of the
#              job so the boot storm overlaps the Rust builds.
#   configure  Wait for the boot to finish, then turn off keyboard extras
#              (autocorrect, predictions, slide-to-type and its first-use
#              tutorials) and unload mediaanalysisd. Needs IOS_UI_UDID from
#              the boot phase.
#   ensure     Wait for CoreSimulator to list the booted device again before
#              a test run, re-booting it if it shut down, and fail with a
#              clear error if it doesn't come back. Needs IOS_UI_UDID.
#
# The boot phase prints GITHUB_ENV lines on stdout; progress goes to stderr:
#   IOS_UI_UDID=<udid>
#   IOS_UI_DESTINATION=platform=iOS Simulator,id=<udid>
set -euo pipefail

cd "$(dirname "$0")/.."

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

    # The simulator's photo/media analysis daemon used 190-380% CPU, on a
    # 3-core runner, for the whole UI test run (run 37251142452). The UI
    # tests don't use Photos. Failing to unload it only costs speed, so warn
    # rather than fail; runner-load.log shows whether it still runs.
    local labels label
    labels=$(mediaanalysis_labels "$udid")
    if [ -z "$labels" ]; then
        echo "No mediaanalysisd job loaded in the simulator" >&2
    fi
    for label in $labels; do
        echo "Unloading $label" >&2
        xcrun simctl spawn "$udid" launchctl remove "$label" >&2 || true
    done
    if [ -n "$labels" ] && [ -n "$(mediaanalysis_labels "$udid")" ]; then
        echo "::warning::mediaanalysisd is still loaded in the simulator" >&2
    fi
}

# In run 38010047883, xcodebuild found no iOS simulators at all ten seconds
# after the keyboard warm-up had used this one, though the simulator kept
# running and simctl reached it a minute later. The runner was busy with
# disk images at the time (diskutil, diskimagesiod), so CoreSimulator likely
# lost sight of the runtime volume for a while. Wait that out here, where the
# cause is clear, instead of failing inside xcodebuild. simctl and xcodebuild
# look devices up differently, so the workflow also retries the test run once
# if xcodebuild still can't find the device.
ensure() {
    local udid=${IOS_UI_UDID:?run the boot phase first}
    local deadline=$((SECONDS + 300)) state rebooted=
    while true; do
        state=$(device_state "$udid")
        case "$state" in
            Booted)
                echo "Simulator $udid is booted" >&2
                return 0
                ;;
            Shutdown)
                if [ -z "$rebooted" ]; then
                    echo "::warning title=Simulator shut down::Simulator $udid shut down before the UI tests; booting it again" >&2
                    xcrun simctl boot "$udid" >&2 || true
                    xcrun simctl bootstatus "$udid" -b >&2 || true
                    rebooted=1
                    continue
                fi
                ;;
        esac
        if [ "$SECONDS" -ge "$deadline" ]; then
            echo "::error title=Simulator unavailable::CoreSimulator lists simulator $udid as '$state' after 5 minutes. This is a runner problem, not a test failure." >&2
            xcrun simctl list devices >&2 || true
            xcrun simctl list runtimes >&2 || true
            return 1
        fi
        echo "Simulator $udid is '$state'; waiting for CoreSimulator to list it as booted" >&2
        sleep 10
    done
}

# Prints the device's state (Booted, Shutdown, ...), "unavailable: <reason>"
# if its runtime is missing, or "missing" if CoreSimulator doesn't list it.
device_state() {
    { xcrun simctl list devices -j 2>/dev/null || true; } | python3 -c '
import json, sys
udid = sys.argv[1]
try:
    runtimes = json.load(sys.stdin)["devices"].values()
except (ValueError, KeyError):
    runtimes = []
for devices in runtimes:
    for d in devices:
        if d["udid"] == udid:
            if d.get("isAvailable", True):
                print(d["state"])
            else:
                print("unavailable: " + d.get("availabilityError", "unknown"))
            sys.exit()
print("missing")
' "$1"
}

mediaanalysis_labels() {
    xcrun simctl spawn "$1" launchctl list | awk 'tolower($3) ~ /mediaanalysis/ { print $3 }'
}

case "${1:-}" in
    boot) boot ;;
    configure) configure ;;
    ensure) ensure ;;
    *)
        echo "usage: $0 boot|configure|ensure" >&2
        exit 2
        ;;
esac

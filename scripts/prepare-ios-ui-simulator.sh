#!/usr/bin/env bash
# Boot and configure a simulator for the iOS UI tests on CI.
#
# A freshly booted simulator saturates a small CI runner for minutes, so CI
# boots early and configures late:
#
#   boot       Pick the newest base-model iPhone (lighter than a Pro Max) and
#              start booting it without waiting. Run this at the start of the
#              job so the boot overlaps the setup steps.
#   configure  Wait for the boot to finish and unload mediaanalysisd. Needs
#              IOS_UI_UDID from the boot phase.
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

    # The simulator's photo/media analysis daemon uses 2-4 cores of a 3-core
    # runner for the whole UI test run, and the UI tests don't use Photos.
    # Failing to unload it only costs speed, so warn rather than fail.
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

mediaanalysis_labels() {
    xcrun simctl spawn "$1" launchctl list | awk 'tolower($3) ~ /mediaanalysis/ { print $3 }'
}

case "${1:-}" in
    boot) boot ;;
    configure) configure ;;
    *)
        echo "usage: $0 boot|configure" >&2
        exit 2
        ;;
esac

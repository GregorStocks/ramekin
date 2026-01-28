#!/usr/bin/env python3
"""Find an available iOS simulator for CI.

Reads JSON from stdin (output of `xcrun simctl list devices available -j`)
and prints the best available iPhone simulator name.

Priority:
1. iPhone 16 (any variant)
2. Any other iPhone
3. Fallback to "iPhone SE (3rd generation)"
"""

import json
import sys


def find_simulator(data: dict) -> str:
    # First pass: look for iPhone 16
    for runtime, devices in data["devices"].items():
        if "iOS" not in runtime:
            continue
        for dev in devices:
            name = dev["name"]
            if "iPhone" in name and "16" in name:
                return name

    # Second pass: any iPhone
    for runtime, devices in data["devices"].items():
        if "iOS" not in runtime:
            continue
        for dev in devices:
            name = dev["name"]
            if "iPhone" in name:
                return name

    # Fallback
    return "iPhone SE (3rd generation)"


if __name__ == "__main__":
    data = json.load(sys.stdin)
    print(find_simulator(data))

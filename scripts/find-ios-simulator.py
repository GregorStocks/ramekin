#!/usr/bin/env python3
"""Find an available iOS simulator for testing.

Reads JSON from stdin (output of `xcrun simctl list devices available -j`)
and prints the UDID of the newest iPhone on the newest iOS runtime.

Returning a UDID (rather than a name) is unambiguous: the same device name
can appear on multiple runtimes (e.g. "iPhone 15" on both iOS 17.2 and iOS
26.4), and `-destination name=...` will pick whichever xcodebuild finds
first.
"""

import json
import re
import sys


def runtime_version(runtime_key: str) -> tuple[int, ...]:
    m = re.search(r"iOS-([\d-]+)$", runtime_key)
    return tuple(int(x) for x in m.group(1).split("-")) if m else ()


def iphone_rank(name: str) -> tuple[int, int, int, str]:
    m = re.search(r"iPhone\s+(\d+)", name)
    model = int(m.group(1)) if m else -1
    is_pro_max = int("Pro Max" in name)
    is_pro = int("Pro" in name and not is_pro_max)
    return (model, is_pro_max, is_pro, name)


def find_simulator(data: dict) -> str:
    runtimes = sorted(
        (k for k in data["devices"] if "iOS" in k),
        key=runtime_version,
        reverse=True,
    )
    for runtime in runtimes:
        iphones = [d for d in data["devices"][runtime] if "iPhone" in d["name"]]
        if iphones:
            return max(iphones, key=lambda d: iphone_rank(d["name"]))["udid"]
    sys.exit("No iPhone simulator available")


if __name__ == "__main__":
    print(find_simulator(json.load(sys.stdin)))

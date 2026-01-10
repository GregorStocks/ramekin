#!/usr/bin/env python3
"""Find domains in paprika that aren't in the known food blogs list."""

import gzip
import json
import re
import zipfile
from collections import Counter
from urllib.parse import urlparse


def parse_known_blogs(rust_file: str) -> set[str]:
    """Extract domain strings from the known_food_blogs array in Rust source."""
    with open(rust_file) as f:
        content = f.read()

    # Find the known_food_blogs array and extract quoted strings
    match = re.search(r"let known_food_blogs = \[(.*?)\];", content, re.DOTALL)
    if not match:
        raise ValueError("Could not find known_food_blogs array in Rust file")

    array_content = match.group(1)
    domains = re.findall(r'"([^"]+)"', array_content)
    return set(domains)


def normalize_domain(host: str) -> str:
    """Normalize domain (remove www., lowercase)."""
    return host.lower().removeprefix("www.")


def get_paprika_domains(archive_path: str) -> Counter[str]:
    """Extract and count domains from paprikarecipes source_url fields."""
    counts: Counter[str] = Counter()

    with zipfile.ZipFile(archive_path) as zf:
        for name in zf.namelist():
            if not name.endswith(".paprikarecipe"):
                continue

            with zf.open(name) as entry:
                compressed = entry.read()
                json_bytes = gzip.decompress(compressed)
                recipe = json.loads(json_bytes)

            source_url = recipe.get("source_url")
            if not source_url:
                continue

            try:
                parsed = urlparse(source_url)
                if parsed.netloc:
                    domain = normalize_domain(parsed.netloc)
                    counts[domain] += 1
            except Exception:
                pass

    return counts


def main():
    known = parse_known_blogs("cli/src/generate_test_urls.rs")
    paprika = get_paprika_domains("data/dev/seed.paprikarecipes")

    unsupported = {d: c for d, c in paprika.items() if d not in known}

    print("=== UNSUPPORTED DOMAINS ===")
    print("(in paprika but not in known list)")
    for domain, count in sorted(unsupported.items(), key=lambda x: -x[1]):
        print(f"{count:>5} | {domain}")

    print(f"\nTotal: {len(unsupported)} domains, {sum(unsupported.values())} recipes")


if __name__ == "__main__":
    main()

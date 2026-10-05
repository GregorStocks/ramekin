#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# ///
"""Serve the AI eval judging pages and save their judgments into the repo.

GET serves logs/ai-evals/ (each suite's judge.html and its images). PUT
/judgments/<suite>.json writes the page's judgments to
data/ai-evals/judgments/<suite>.json, so judging works from another machine
without copying the download back.
"""

from __future__ import annotations

import argparse
import functools
import json
import re
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGES = ROOT / "logs" / "ai-evals"
JUDGMENTS = ROOT / "data" / "ai-evals" / "judgments"
SAVE_PATH = re.compile(r"^/judgments/([a-z-]+)\.json$")
SUITES = {"tags", "titles", "descriptions", "custom-enrich", "recipe-photos"}
MAX_BYTES = 5_000_000


class Handler(SimpleHTTPRequestHandler):
    def do_PUT(self) -> None:  # noqa: N802 (http.server's naming)
        match = SAVE_PATH.match(self.path)
        if not match or match.group(1) not in SUITES:
            self.send_error(404, "Not a judged suite")
            return
        length = int(self.headers.get("Content-Length", "0"))
        if not 0 < length <= MAX_BYTES:
            self.send_error(400, "Bad length")
            return
        body = self.rfile.read(length)
        try:
            judgments = json.loads(body)
        except ValueError:
            self.send_error(400, "Not JSON")
            return
        if not isinstance(judgments, dict):
            self.send_error(400, "Judgments must be an object")
            return
        JUDGMENTS.mkdir(parents=True, exist_ok=True)
        path = JUDGMENTS / f"{match.group(1)}.json"
        path.write_text(
            json.dumps(judgments, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
        )
        self.send_response(204)
        self.end_headers()
        print(f"Saved {path.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    handler = functools.partial(Handler, directory=str(PAGES))
    server = ThreadingHTTPServer((args.host, args.port), handler)
    for page in sorted(PAGES.glob("*/judge.html")):
        print(f"http://{args.host}:{args.port}/{page.parent.name}/judge.html")
    server.serve_forever()


if __name__ == "__main__":
    main()

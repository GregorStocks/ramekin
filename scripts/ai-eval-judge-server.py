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
import os
import re
import socket
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGES = ROOT / "logs" / "ai-evals"
JUDGMENTS = ROOT / "data" / "ai-evals" / "judgments"
SAVE_PATH = re.compile(r"^/judgments/([a-z-]+)\.json$")
SUITES = {"tags", "titles", "descriptions", "edit-recipe", "recipe-photos"}
MAX_BYTES = 5_000_000
# A save reads, merges and writes the file, so two at once could lose one.
SAVE_LOCK = threading.Lock()


def merge(on_disk: dict, saved: dict) -> dict:
    """A page's judgments merged into the file's.

    Per case, a verdict suite's answers are merged, so a page opened before
    another page saved can't drop that page's verdicts; a tag set is replaced.
    """
    merged = dict(on_disk)
    for case, judgments in saved.items():
        current = merged.get(case)
        if isinstance(current, dict) and isinstance(judgments, dict):
            merged[case] = {**current, **judgments}
        else:
            merged[case] = judgments
    return merged


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
        with SAVE_LOCK:
            on_disk = json.loads(path.read_text()) if path.exists() else {}
            merged = merge(on_disk, judgments)
            path.write_text(
                json.dumps(merged, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
            )
        self.send_response(204)
        self.end_headers()
        print(f"Saved {path.relative_to(ROOT)}")


def default_host() -> str:
    """The address this SSH session came in on, so the printed links open
    from the machine you're sshing from; else localhost."""
    fields = os.environ.get("SSH_CONNECTION", "").split()
    return fields[2] if len(fields) == 4 else "127.0.0.1"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default=default_host())
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    handler = functools.partial(Handler, directory=str(PAGES))
    ipv6 = ":" in args.host
    server_class = type(
        "Server",
        (ThreadingHTTPServer,),
        {"address_family": socket.AF_INET6 if ipv6 else socket.AF_INET},
    )
    server = server_class((args.host, args.port), handler)
    url_host = f"[{args.host}]" if ipv6 else args.host
    for page in sorted(PAGES.glob("*/judge.html")):
        print(f"http://{url_host}:{args.port}/{page.parent.name}/judge.html")
    server.serve_forever()


if __name__ == "__main__":
    main()

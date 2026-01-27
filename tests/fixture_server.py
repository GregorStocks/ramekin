#!/usr/bin/env python3
"""HTTP server for test fixtures with silent logging."""

import sys
from http.server import HTTPServer, SimpleHTTPRequestHandler


class SilentHandler(SimpleHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def log_error(self, format, *args):
        print(format % args, file=sys.stderr)


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    server = HTTPServer(("", port), SilentHandler)
    server.serve_forever()

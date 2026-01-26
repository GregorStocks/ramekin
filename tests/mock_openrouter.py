#!/usr/bin/env python3
"""Mock OpenRouter server for testing.

Returns valid OpenAI-compatible chat completion responses.
"""

import json
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler


class MockOpenRouterHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        # Health check endpoint
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"status": "ok"}')

    def do_POST(self):
        if self.path == "/v1/chat/completions":
            # Read request body
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)

            try:
                request = json.loads(body)
            except json.JSONDecodeError:
                self.send_error(400, "Invalid JSON")
                return

            # Return a mock response with empty suggested_tags
            response = {
                "id": "mock-completion-id",
                "object": "chat.completion",
                "created": 1234567890,
                "model": request.get("model", "mock-model"),
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": '{"suggested_tags": []}',
                        },
                        "finish_reason": "stop",
                    }
                ],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15,
                },
            }

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(response).encode())
        else:
            self.send_error(404, "Not found")

    def log_message(self, format, *args):
        # Log to stderr so it appears in process-compose logs
        print(f"[mock-openrouter] {args[0]}", file=sys.stderr)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 3003
    server = HTTPServer(("", port), MockOpenRouterHandler)
    print(f"Mock OpenRouter server running on port {port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()

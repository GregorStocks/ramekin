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

            content = self._generate_response_content(request)

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
                            "content": content,
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

    def _generate_response_content(self, request):
        """Generate appropriate mock response based on the request type."""
        messages = request.get("messages", [])
        all_text = " ".join(m.get("content", "") for m in messages)

        # Custom enrich: extract the recipe from the request and return it
        # with a small modification to prove the "change" was applied
        if "recipe modification assistant" in all_text:
            return self._mock_custom_enrich(all_text)

        # Default: auto-tag response
        return '{"suggested_tags": ["test-auto-tag"]}'

    def _mock_custom_enrich(self, all_text):
        """Return a modified recipe for custom enrich requests."""
        # Try to extract the recipe JSON from the prompt
        try:
            # The recipe JSON is between "Here is the recipe:" and "Apply this change:"
            start = all_text.index("Here is the recipe:") + len("Here is the recipe:")
            end = all_text.index("Apply this change:")
            recipe_json = all_text[start:end].strip()
            recipe = json.loads(recipe_json)
            # Apply a visible modification: prepend "[Modified] " to the title
            recipe["title"] = "[Modified] " + recipe.get("title", "")
            return json.dumps(recipe)
        except (ValueError, json.JSONDecodeError):
            # Fallback: return a minimal valid recipe
            return json.dumps(
                {
                    "title": "[Modified] Test Recipe",
                    "instructions": "Modified instructions.",
                    "ingredients": [],
                    "tags": [],
                }
            )

    def log_message(self, format, *args):
        pass


def main():
    if len(sys.argv) < 2:
        print("Error: Port argument required", file=sys.stderr)
        print("Usage: python mock_openrouter.py <port>", file=sys.stderr)
        sys.exit(1)
    port = int(sys.argv[1])
    server = HTTPServer(("", port), MockOpenRouterHandler)
    print(f"Mock OpenRouter server running on port {port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()

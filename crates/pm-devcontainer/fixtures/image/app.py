"""Serves a greeting on the container's loopback, the way a development server would."""

import http.server
import os
import sys


class Greeting(http.server.BaseHTTPRequestHandler):
    """Answers every request with the fixture's name."""

    def do_GET(self):
        """Writes the greeting."""
        body = f"hello from {os.environ.get('PANDEMONIUM_FIXTURE', 'unknown')}\n".encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == "__main__":
    http.server.HTTPServer(("127.0.0.1", 8080), Greeting).serve_forever()

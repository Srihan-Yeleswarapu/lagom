#!/usr/bin/env python3
"""Serve web/ locally for the Lagom playground.

Usage: python3 scripts/serve-web.py [port]     (default 8765)

The playground pauses a program on each `ask` by sharing memory between the
page and its worker (SharedArrayBuffer). Browsers only allow that on a
cross-origin-isolated page, so this server sends the isolation headers.
Plain `python3 -m http.server` will not work for interactive input.
"""
import http.server
import os
import sys
import urllib.parse

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "web")


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".mjs": "text/javascript",
    }

    # Lesson and practice URLs are served by one HTML shell each. The page reads
    # its own id from location.pathname.
    def translate_path(self, path):
        clean = urllib.parse.urlsplit(path).path.rstrip("/") or "/"
        if clean == "/practice":
            path = "/practice.html"
        elif clean.startswith("/lesson/"):
            path = "/lesson.html"
        return super().translate_path(path)

    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
    handler = lambda *args, **kwargs: Handler(*args, directory=ROOT, **kwargs)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    print(f"Serving {os.path.abspath(ROOT)} at http://localhost:{port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Fetch the Sequence OpenAPI spec into docs/openapi.yaml (gitignored).

The spec is Sequence's, not ours, so it isn't committed. Run this once for
local dev. Override the source with an arg or SEQUENCE_OPENAPI_URL.

    python3 scripts/fetch_openapi.py [url]
"""

import os
import sys
import urllib.request
from pathlib import Path

# The spec the Scalar docs at app.getsequence.io/api/platform load.
# Override via arg or SEQUENCE_OPENAPI_URL.
DEFAULT_URL = "https://app.getsequence.io/api/platform/v1/openapi"
DEST = Path(__file__).resolve().parent.parent / "docs" / "openapi.yaml"


def main() -> int:
    url = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("SEQUENCE_OPENAPI_URL", DEFAULT_URL)
    print(f"fetching {url}")
    # A plain urllib User-Agent is WAF-blocked (403); a browser-style one passes.
    req = urllib.request.Request(
        url, headers={"User-Agent": "Mozilla/5.0 (compatible; sequence-rs fetch_openapi)"}
    )
    try:
        with urllib.request.urlopen(req) as resp:
            data = resp.read()
    except Exception as e:  # noqa: BLE001 — quick-and-dirty dev script
        print(f"error: {e}", file=sys.stderr)
        return 1

    DEST.parent.mkdir(parents=True, exist_ok=True)
    DEST.write_bytes(data)
    print(f"wrote {len(data)} bytes to {DEST}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Legacy dev helper: same JSON protocol as the old Rust↔Python bridge.

The app embeds text in-process via ONNX (fastembed). This script is not used by release builds.
Read one JSON object per line from stdin: {\"texts\": [\"...\", ...]}.
Emit one JSON line: {\"dim\": 384, \"vectors\": [[f32, ...], ...]}.
"""

from __future__ import annotations

import json
import sys


def _load_model():
    """Import and load once per process (after stdin is open so Rust can write the first batch)."""
    from sentence_transformers import SentenceTransformer

    return SentenceTransformer("all-MiniLM-L6-v2")


def main() -> None:
    model = None

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as e:
            print(json.dumps({"error": f"json: {e}"}), flush=True)
            sys.exit(1)

        texts = obj.get("texts")
        if not isinstance(texts, list) or not texts:
            print(json.dumps({"error": "missing texts array"}), flush=True)
            sys.exit(1)

        if model is None:
            try:
                model = _load_model()
            except ImportError as e:
                print(json.dumps({"error": f"import failed: {e}"}), flush=True)
                sys.exit(1)

        emb = model.encode(texts, normalize_embeddings=True)
        vecs = emb.tolist()
        dim = len(vecs[0]) if vecs else 384
        print(json.dumps({"dim": dim, "vectors": vecs}), flush=True)


if __name__ == "__main__":
    main()

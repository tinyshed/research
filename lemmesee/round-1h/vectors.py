"""Round 1h's word vectors: the 20 000 commonest words of SchemaPile-Perm's
table and column names, each embedded offline by a small sentence-embedding
model on a graphics card, cut to 16 numbers by principal components and
kept as one byte each with a scale per number, as the look would carry
them.

    python -I vectors.py vocab <schemapile-perm.json.gz> <out> [<words>]     (round 1b's environment)
    python -I vectors.py embed <out> [<model>]                                (round 1h's, with torch)

vocab writes <out>/vectors/vocab.json; embed writes <out>/vectors/vectors.json
— the words, the scales and the bytes — and reports its size packed.
"""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

HERE = Path(__file__).parent
DIMS = 16
MODEL = "BAAI/bge-small-en-v1.5"


def vocab(source: Path, out: Path, n: int = 20000) -> None:
    sys.path.insert(0, str(HERE.parent / "round-1g"))
    import lexicon
    lexicon.vocab(source, out / "vectors-tmp", n)
    (out / "vectors").mkdir(parents=True, exist_ok=True)
    (out / "vectors-tmp" / "lexicon" / "vocab.json").replace(out / "vectors" / "vocab.json")
    (out / "vectors-tmp" / "lexicon").rmdir()
    (out / "vectors-tmp").rmdir()


def embed(out: Path, model_name: str = MODEL) -> None:
    import numpy as np
    from sentence_transformers import SentenceTransformer
    words = json.loads((out / "vectors" / "vocab.json").read_text(encoding="utf-8"))["words"]
    model = SentenceTransformer(model_name, device="cuda")
    E = model.encode(words, batch_size=512, normalize_embeddings=True, show_progress_bar=False)
    E = E - E.mean(axis=0)
    # principal components: the 16 directions the words vary along most
    _, S, Vt = np.linalg.svd(E, full_matrices=False)
    P = E @ Vt[:DIMS].T
    kept = float((S[:DIMS] ** 2).sum() / (S ** 2).sum())
    scale = np.abs(P).max(axis=0) / 127.0
    Q = np.round(P / scale).astype(np.int8)
    (out / "vectors" / "vectors.json").write_text(json.dumps({
        "model": model_name, "dims": DIMS, "variance kept": round(kept, 3), "words": words,
        "scale": [float(x) for x in scale], "bytes": base64.b64encode(Q.tobytes()).decode()}), encoding="utf-8")
    packed = sum(1 + len(w.encode()) for w in words) + Q.nbytes + 4 * DIMS
    print(f"{len(words)} words, {DIMS} numbers each keeping {kept:.1%} of the variance; about {packed} bytes packed")


def load(out: Path) -> dict:
    """A word to its 16 numbers, as the look reads them."""
    import numpy as np
    d = json.loads((out / "vectors" / "vectors.json").read_text(encoding="utf-8"))
    Q = np.frombuffer(base64.b64decode(d["bytes"]), dtype=np.int8).reshape(-1, d["dims"]).astype(np.float32)
    V = Q * np.array(d["scale"], dtype=np.float32)
    return {w: V[i] for i, w in enumerate(d["words"])}


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "vocab":
        vocab(Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4]) if len(sys.argv) > 4 else 20000)
    else:
        embed(Path(sys.argv[2]), sys.argv[3] if len(sys.argv) > 3 else MODEL)

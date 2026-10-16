"""Embeds what `lab export` wrote with BAAI/bge-m3's dense vectors.

python embed.py <export dir> <model dir>

Writes <export dir>/docs.f32 and questions.f32, one row of 1024
little-endian floats a passage or question in the export's order, each of
length one, and embed.json with the model's revision, the versions, the
tokens and the seconds. Passages go in parts of 50,000, each kept as it
ends, so that a run stopped halfway starts again where it stopped.
"""

import json
import sys
import time
from pathlib import Path

import numpy as np
import sentence_transformers
import torch
import transformers
from huggingface_hub import snapshot_download

MODEL = "BAAI/bge-m3"
REVISION = "5617a9f61b028005a4858fdac845db406aefb181"
PART = 50_000
LONGEST = 512

out = Path(sys.argv[1])
model_dir = Path(sys.argv[2])

snapshot_download(MODEL, revision=REVISION, local_dir=model_dir,
                  allow_patterns=["*.json", "pytorch_model.bin", "sentencepiece.bpe.model", "1_Pooling/*"])
model = sentence_transformers.SentenceTransformer(str(model_dir), device="cuda")
model.half()
model.max_seq_length = LONGEST


def passages():
    with open(out / "docs.jsonl", encoding="utf-8") as lines:
        for line in lines:
            doc = json.loads(line)
            yield f"{doc['title']}\n{doc['text']}"


def tokens(texts):
    counted = model.tokenizer(texts, truncation=True, max_length=LONGEST)["input_ids"]
    return sum(len(ids) for ids in counted)


def embed(texts):
    return model.encode(texts, batch_size=128, normalize_embeddings=True, convert_to_numpy=True).astype("<f4")


parts = out / "parts"
parts.mkdir(exist_ok=True)
texts = list(passages())
log = {"model": MODEL, "revision": REVISION, "longest_tokens": LONGEST, "precision": "float16",
       "torch": torch.__version__, "transformers": transformers.__version__,
       "sentence_transformers": sentence_transformers.__version__, "gpu": torch.cuda.get_device_name(0),
       "documents": len(texts), "parts": []}
for at in range(0, len(texts), PART):
    path = parts / f"{at // PART:03}.npy"
    if path.exists():
        continue
    chunk = texts[at:at + PART]
    started = time.perf_counter()
    vectors = embed(chunk)
    seconds = time.perf_counter() - started
    line = {"part": at // PART, "seconds": seconds, "tokens": tokens(chunk)}
    path.with_suffix(".json").write_text(json.dumps(line))
    np.save(path, vectors)
    print(json.dumps(line), flush=True)

with open(out / "docs.f32", "wb") as docs:
    for at in range(0, len(texts), PART):
        docs.write(np.load(parts / f"{at // PART:03}.npy").tobytes())
        log["parts"].append(json.loads((parts / f"{at // PART:03}.json").read_text()))

questions = [json.loads(line)["text"] for line in open(out / "questions.jsonl", encoding="utf-8")]
started = time.perf_counter()
asked = embed(questions)
log["questions"] = {"count": len(questions), "seconds": time.perf_counter() - started, "tokens": tokens(questions)}
(out / "questions.f32").write_bytes(asked.tobytes())
(out / "embed.json").write_text(json.dumps(log, indent=1))
print(json.dumps({"step": "embedded", "tokens": sum(p["tokens"] for p in log["parts"]),
                  "seconds": sum(p["seconds"] for p in log["parts"])}))

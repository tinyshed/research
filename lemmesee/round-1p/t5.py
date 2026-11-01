"""Round 1p's baseline: SchemaPile's own T5 foreign-key model on the same
pairs as round 1b's model, run here rather than quoted from the paper.

    <python with torch> -I t5.py <model dir> <out>

reads hidden.jsonl and bench.jsonl from <out> and writes t5-outcomes.jsonl
and t5.json there. The model is `tdoehmen/t5-schemapile-fk`, a T5-base
fine-tuned on SchemaPile's foreign keys; it is run by a forward pass written
here on torch alone, its weights read as saved. Each pair is answered twice:

- constrained: every (column, referenced column) the two tables allow is
  scored by the model's log-likelihood of writing it, and the best is its
  answer, which is how the paper's notebook constrained StarCoder;
- free: greedy decoding, parsed as the notebook parses T5's output, '{' +
  text + '}', and compared field by field as its `evaluate_responses` does.
  The notebook decoded with eight beams; greedy is the cheaper reading.
"""

import json
import math
import sys
import time
from pathlib import Path

import torch
from tokenizers import Tokenizer

QUESTION = ("You are given the following SQL database tables: \n{tables}\nOutput a json string with the "
            "following schema {{table, column, referencedTable, referencedColumn}} that contains the foreign "
            "key relationship between the two tables.")
MAX_INPUT = 1024
MAX_OUTPUT = 160
CHUNK = 48


class T5:
    """T5 v1.0 as Hugging Face's T5ForConditionalGeneration computes it:
    RMS layer norms, unscaled attention with a relative position bias from
    each stack's first layer, ReLU feed-forward, tied output embedding."""

    def __init__(self, model: Path, device: str):
        cfg = json.load(open(model / "config.json", encoding="utf-8"))
        assert cfg.get("feed_forward_proj", "relu") == "relu" and cfg.get("is_encoder_decoder", True)
        weights = torch.load(model / "pytorch_model.bin", map_location="cpu", weights_only=True)
        self.w = {k: v.float().to(device) for k, v in weights.items()}
        self.heads, self.d_kv, self.eps = cfg["num_heads"], cfg["d_kv"], cfg["layer_norm_epsilon"]
        self.buckets = cfg["relative_attention_num_buckets"]
        self.max_distance = cfg.get("relative_attention_max_distance", 128)
        self.layers, self.decoder_layers = cfg["num_layers"], cfg.get("num_decoder_layers", cfg["num_layers"])
        self.scale = cfg["d_model"] ** -0.5 if cfg.get("tie_word_embeddings", True) else 1.0
        self.embed = self.w["shared.weight"]
        self.head = self.w.get("lm_head.weight", self.embed) if self.scale == 1.0 else self.embed
        self.device = device

    def norm(self, x, name):
        return x * torch.rsqrt(x.pow(2).mean(-1, keepdim=True) + self.eps) * self.w[name]

    def bucket(self, relative, bidirectional):
        n, out = self.buckets, torch.zeros_like(relative)
        if bidirectional:
            n //= 2
            out += (relative > 0).long() * n
            relative = relative.abs()
        else:
            relative = -torch.min(relative, torch.zeros_like(relative))
        exact = n // 2
        large = exact + (torch.log(relative.float().clamp(min=1) / exact) / math.log(self.max_distance / exact)
                         * (n - exact)).long()
        large = torch.min(large, torch.full_like(large, n - 1))
        return out + torch.where(relative < exact, relative, large)

    def position_bias(self, q, k, name, bidirectional):
        context = torch.arange(q, device=self.device)[:, None]
        memory = torch.arange(k, device=self.device)[None, :]
        values = self.w[name][self.bucket(memory - context, bidirectional)]  # q, k, heads
        return values.permute(2, 0, 1)[None]

    def split(self, x):
        b, t, _ = x.shape
        return x.view(b, t, self.heads, self.d_kv).transpose(1, 2)

    def attend(self, x, prefix, bias, kv=None, cached=None):
        q = self.split(x @ self.w[prefix + "q.weight"].T)
        if cached is None:
            src = x if kv is None else kv
            k = self.split(src @ self.w[prefix + "k.weight"].T)
            v = self.split(src @ self.w[prefix + "v.weight"].T)
        else:
            k, v = cached
        scores = q @ k.transpose(-1, -2) + bias
        out = torch.softmax(scores, dim=-1) @ v
        b, _, t, _ = out.shape
        return out.transpose(1, 2).reshape(b, t, -1) @ self.w[prefix + "o.weight"].T

    def ff(self, x, prefix):
        return torch.relu(x @ self.w[prefix + "wi.weight"].T) @ self.w[prefix + "wo.weight"].T

    @torch.no_grad()
    def encode(self, ids):
        x = self.embed[ids][None]
        t = x.shape[1]
        bias = self.position_bias(t, t, "encoder.block.0.layer.0.SelfAttention.relative_attention_bias.weight", True)
        for i in range(self.layers):
            p = f"encoder.block.{i}.layer."
            h = self.norm(x, p + "0.layer_norm.weight")
            x = x + self.attend(h, p + "0.SelfAttention.", bias)
            x = x + self.ff(self.norm(x, p + "1.layer_norm.weight"), p + "1.DenseReluDense.")
        enc = self.norm(x, "encoder.final_layer_norm.weight")
        # The cross-attention keys and values of every decoder layer, once
        cross = []
        for i in range(self.decoder_layers):
            p = f"decoder.block.{i}.layer.1.EncDecAttention."
            cross.append((self.split(enc @ self.w[p + "k.weight"].T), self.split(enc @ self.w[p + "v.weight"].T)))
        return cross

    @torch.no_grad()
    def logits(self, dec, cross):
        b, t = dec.shape
        x = self.embed[dec]
        bias = self.position_bias(t, t, "decoder.block.0.layer.0.SelfAttention.relative_attention_bias.weight", False)
        causal = torch.full((t, t), torch.finfo(x.dtype).min, device=self.device).triu(1)
        bias = bias + causal[None, None]
        zero = torch.zeros(1, 1, 1, 1, device=self.device)
        for i in range(self.decoder_layers):
            p = f"decoder.block.{i}.layer."
            x = x + self.attend(self.norm(x, p + "0.layer_norm.weight"), p + "0.SelfAttention.", bias)
            k, v = cross[i]
            x = x + self.attend(self.norm(x, p + "1.layer_norm.weight"), p + "1.EncDecAttention.", zero,
                                cached=(k.expand(b, -1, -1, -1), v.expand(b, -1, -1, -1)))
            x = x + self.ff(self.norm(x, p + "2.layer_norm.weight"), p + "2.DenseReluDense.")
        x = self.norm(x, "decoder.final_layer_norm.weight") * self.scale
        return x @ self.head.T

    def score(self, cross, targets: list[list[int]]) -> list[float]:
        """Each target's log-likelihood, teacher forced from the pad token."""
        out = []
        for start in range(0, len(targets), CHUNK):
            chunk = targets[start:start + CHUNK]
            t = max(len(x) for x in chunk)
            labels = torch.zeros(len(chunk), t, dtype=torch.long, device=self.device)
            mask = torch.zeros(len(chunk), t, device=self.device)
            for i, x in enumerate(chunk):
                labels[i, :len(x)] = torch.tensor(x, device=self.device)
                mask[i, :len(x)] = 1
            dec = torch.cat([torch.zeros(len(chunk), 1, dtype=torch.long, device=self.device), labels[:, :-1]], 1)
            logits = self.logits(dec, cross)
            logp = logits.gather(-1, labels[..., None])[..., 0] - torch.logsumexp(logits, -1)
            del logits
            out.extend((logp * mask).sum(1).tolist())
        return out

    def greedy(self, cross) -> list[int]:
        dec = [0]
        for _ in range(MAX_OUTPUT):
            nxt = int(self.logits(torch.tensor([dec], device=self.device), cross)[0, -1].argmax())
            dec.append(nxt)
            if nxt == 1:
                break
        return dec[1:]


def table_string(t: dict) -> str:
    return t["name"].lower() + "(" + ", ".join(c["name"].lower() for c in t["columns"]) + ")"


def answer(table, column, foreign, referred) -> str:
    return json.dumps({"table": table.lower(), "column": column.lower(), "referencedTable": foreign.lower(),
                       "referencedColumn": referred.lower()})


def main(model: Path, out: Path) -> None:
    device = "cuda" if torch.cuda.is_available() else "cpu"
    t0 = time.time()
    t5 = T5(model, device)
    tok = Tokenizer.from_file(str(model / "tokenizer.json"))
    tok.enable_truncation(MAX_INPUT)
    schemas = {}
    for line in open(out / "hidden.jsonl", encoding="utf-8"):
        s = json.loads(line)
        schemas[s["id"]] = {t["name"]: t for t in s["tables"]}
    bench = [json.loads(line) for line in open(out / "bench.jsonl", encoding="utf-8")]
    print(f"loaded on {device} in {time.time() - t0:.0f} s; {len(bench)} pairs", flush=True)

    # A run stopped part way, or one over fewer databases, goes on from the
    # pairs it wrote, each known by what it asks
    key = lambda p: (p["source"], p["db"], p["table"], p["foreign_table"], p["column"], p["referred_column"])  # noqa: E731
    partial = out / "t5-outcomes.partial.jsonl"
    done = {key(o): o for o in map(json.loads, open(partial, encoding="utf-8"))} if partial.exists() else {}
    outcomes, seconds = [], []
    sink = open(partial, "a", encoding="utf-8")
    for j, p in enumerate(bench):
        if key(p) in done:
            outcomes.append({**done[key(p)], "novel": p["novel"], "occurs_in_perm": p["occurs_in_perm"],
                             "too_large": p["too_large"], "composite": p["composite"]})
            seconds.append(outcomes[-1]["seconds"])
            continue
        tables = schemas[f"{p['source']}/{p['db']}"]
        a, b = tables[p["table"]], tables[p["foreign_table"]]
        prompt = QUESTION.format(tables="\n".join([table_string(a), table_string(b)]))
        truth = json.loads(answer(p["table"], p["column"], p["foreign_table"], p["referred_column"]))
        s0 = time.time()
        cross = t5.encode(torch.tensor(tok.encode(prompt).ids, device=device))
        options = [(c["name"], r["name"]) for c in a["columns"] for r in b["columns"]]
        scores = t5.score(cross, [tok.encode(answer(a["name"], c, b["name"], r)).ids for c, r in options])
        best = options[max(range(len(options)), key=lambda i: scores[i])]
        text = tok.decode(t5.greedy(cross), skip_special_tokens=True)
        try:
            free = json.loads("{" + text + "}")
        except json.JSONDecodeError:
            free = None
        seconds.append(time.time() - s0)
        outcomes.append({
            "source": p["source"], "db": p["db"], "table": p["table"], "foreign_table": p["foreign_table"],
            "column": p["column"], "referred_column": p["referred_column"],
            "occurs_in_perm": p["occurs_in_perm"], "too_large": p["too_large"], "composite": p["composite"],
            "novel": p["novel"], "options": len(options), "constrained": list(best),
            "hit constrained": best[0].lower() == p["column"].lower() and best[1].lower() == p["referred_column"].lower(),
            "free": text, "hit free": free is not None and list(free.keys()) == list(truth.keys())
            and list(free.values()) == list(truth.values()), "seconds": seconds[-1],
        })
        sink.write(json.dumps(outcomes[-1], ensure_ascii=False) + "\n")
        sink.flush()
        if j % 100 == 0:
            print(f"{j}/{len(bench)} {time.time() - t0:.0f} s", flush=True)
    sink.close()
    with open(out / "t5-outcomes.jsonl", "w", encoding="utf-8") as f:
        for o in outcomes:
            f.write(json.dumps(o, ensure_ascii=False) + "\n")
    summary = {"device": device, "torch": torch.__version__, "seconds": round(time.time() - t0),
               "median_seconds_a_pair": sorted(seconds)[len(seconds) // 2]}
    for src in ("spider", "bird", "ctu"):
        kept = [o for o in outcomes if o["source"] == src and not o["occurs_in_perm"] and not o["too_large"]]
        summary[src] = {"pairs": len(kept),
                        "constrained": sum(o["hit constrained"] for o in kept) / max(len(kept), 1),
                        "free": sum(o["hit free"] for o in kept) / max(len(kept), 1)}
    (out / "t5.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2))


def negatives(model: Path, out: Path) -> None:
    """How sure T5 is of its best answer for two tables that hold a key,
    and for two tables of the same database that hold none either way: for
    each pair the paper's filters keep, one table of its database chosen at
    random with no declared key between it and the pair's first table. The
    open setting needs a model to tell the two apart; the pair setting never
    asks it to."""
    import random
    device = "cuda" if torch.cuda.is_available() else "cpu"
    t5 = T5(model, device)
    tok = Tokenizer.from_file(str(model / "tokenizer.json"))
    tok.enable_truncation(MAX_INPUT)
    schemas = {}
    for line in open(out / "hidden.jsonl", encoding="utf-8"):
        s = json.loads(line)
        schemas[s["id"]] = s
    bench = [json.loads(line) for line in open(out / "bench.jsonl", encoding="utf-8")]
    rng = random.Random(20261101)
    jobs = []
    for p in bench:
        if p["occurs_in_perm"] or p["too_large"]:
            continue
        s = schemas[f"{p['source']}/{p['db']}"]
        linked = {(a, b) for a, _, b in s["fk"]} | {(b, a) for a, _, b in s["fk"]}
        others = [t["name"] for t in s["tables"] if t["name"] not in (p["table"], p["foreign_table"])
                  and (p["table"], t["name"]) not in linked]
        jobs.append((p, p["foreign_table"], True))
        if others:
            jobs.append((p, rng.choice(others), False))
    rows = []
    t0 = time.time()
    for j, (p, other, related) in enumerate(jobs):
        tables = {t["name"]: t for t in schemas[f"{p['source']}/{p['db']}"]["tables"]}
        a, b = tables[p["table"]], tables[other]
        prompt = QUESTION.format(tables="\n".join([table_string(a), table_string(b)]))
        cross = t5.encode(torch.tensor(tok.encode(prompt).ids, device=device))
        options = [(c["name"], r["name"]) for c in a["columns"] for r in b["columns"]]
        scores = t5.score(cross, [tok.encode(answer(a["name"], c, b["name"], r)).ids for c, r in options])
        rows.append({"source": p["source"], "db": p["db"], "table": p["table"], "other": other,
                     "related": related, "novel": p["novel"], "best_log_likelihood": max(scores)})
        if j % 200 == 0:
            print(f"{j}/{len(jobs)} {time.time() - t0:.0f} s", flush=True)
    with open(out / "t5-negatives.jsonl", "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"{len(rows)} pairs in {time.time() - t0:.0f} s")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "negatives":
        negatives(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        main(Path(sys.argv[1]), Path(sys.argv[2]))

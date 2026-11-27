"""Round 1z, part A: small neural pair scorers on lemmekey's training rows.

    python -I net.py fit <name>                 char-s, char-m or granite; on the graphics card
    python -I net.py score <name> <set.pkl> <out.npy>   a frozen network's scores of a set's pairs
    python -I net.py quantise <name>            8-bit linear layers: bytes, and F1 on the validation split
    python -I net.py time <name> <out.json> <set.pkl> [...]   seconds a database on one thread

Runs in the round's own environment (PyTorch). A set is a pickle
{"schemas": [...], "pairs": {"schema", "table", "column", "target", "key",
"label"}} as round 1v's first.py keeps them; training reads
corpus/round-1v/first/eleven/train.pkl and its mask `sample`, validation
that folder's validation.pkl.

A pair is two sides. The column's side: its name, type, table. The
target's side: the table, its key column, the key's type. `char-*` reads
their bytes in fixed slots of one sequence; `granite` reads them as two
segments of text. Ten flags and counts go beside (char) or into the text
(granite).
"""

import hashlib
import json
import math
import pickle
import sys
import time
from pathlib import Path

import numpy as np
import torch
import torch.nn as nn

HERE = Path(__file__).resolve().parent
CORPUS = HERE.parent / "corpus"
OUT = CORPUS / "round-1z" / "nets"
FIRST = CORPUS / "round-1v" / "first" / "eleven"
SEED = 20261027
STOP = CORPUS / "round-1z" / "stop"  # the round is paused while this file exists

GRANITE = "ibm-granite/granite-embedding-small-english-r2"
GRANITE_REV = "2ab6fa8ea2d674564defd37171ae19079b864b33"
CONFIGS = {
    "char-s": {"kind": "char", "dim": 128, "layers": 4, "heads": 4, "ff": 256, "epochs": 10, "batch": 1024,
               "lr": 1e-3},
    "char-m": {"kind": "char", "dim": 256, "layers": 6, "heads": 8, "ff": 512, "epochs": 10, "batch": 1024,
               "lr": 1e-3},
    "granite": {"kind": "text", "epochs": 1, "batch": 128, "lr": 5e-5},
}
# the bytes each field may take; with six separators the sequence is 128
FIELDS_LEFT = (("column", 30), ("type", 10), ("table", 26))
FIELDS_RIGHT = (("target", 26), ("key", 22), ("ktype", 8))
PAD, CLS = 256, 257
SEP0 = 258  # 258.. one separator a field, so a field's kind is in its first position
LEFT_LEN = 1 + sum(n + 1 for _, n in FIELDS_LEFT)
RIGHT_LEN = sum(n + 1 for _, n in FIELDS_RIGHT)
TEXT_LEFT, TEXT_RIGHT = 56, 37  # tokens a side for granite; with three specials 96


# ---------------------------------------------------------------- the pairs

def sides(schemas: list, rec: dict, rows: np.ndarray | None = None):
    """The unique column sides and target sides of the pairs, each pair's
    index into them, and its ten flags and counts."""
    sc, tb, cl, tg, ky = (rec[k] if rows is None else rec[k][rows] for k in ("schema", "table", "column", "target", "key"))
    lk = (sc.astype(np.int64) << 32) | (tb.astype(np.int64) << 16) | cl.astype(np.int64)
    rk = (sc.astype(np.int64) << 32) | (tg.astype(np.int64) << 16) | ky.astype(np.int64)
    lu, li = np.unique(lk, return_inverse=True)
    ru, ri = np.unique(rk, return_inverse=True)
    left, right = [], []
    lf = np.zeros((len(lu), 7), np.float32)
    rf = np.zeros((len(ru), 2), np.float32)
    lfam, rfam = [], []
    for j, k in enumerate(lu):
        s = schemas[int(k >> 32)]
        t = s["tables"][int((k >> 16) & 0xFFFF)]
        c = t["columns"][int(k & 0xFFFF)]
        left.append({"column": c["name"], "type": c.get("type") or "", "table": t["name"],
                     "nullable": c.get("nullable"), "unique": bool(c.get("unique")), "primary": bool(c.get("primary")),
                     "index": bool(c.get("index")), "position": int(c.get("position") or 0), "columns": len(t["columns"]),
                     "tables": len(s["tables"])})
        lf[j] = [0.5 if c.get("nullable") is None else float(bool(c["nullable"])), float(bool(c.get("unique"))),
                 float(bool(c.get("primary"))), float(bool(c.get("index"))), min(int(c.get("position") or 0), 40) / 40,
                 math.log1p(len(t["columns"])) / 5, math.log1p(len(s["tables"])) / 6]
        lfam.append(c.get("family") or "")
    for j, k in enumerate(ru):
        s = schemas[int(k >> 32)]
        t = s["tables"][int((k >> 16) & 0xFFFF)]
        c = t["columns"][int(k & 0xFFFF)]
        whole_pk = list(t.get("pk") or []) == [c["name"]]
        right.append({"target": t["name"], "key": c["name"], "ktype": c.get("type") or "", "whole": whole_pk})
        rf[j] = [float(whole_pk), math.log1p(len(t["columns"])) / 5]
        rfam.append(c.get("family") or "")
    lfam, rfam = np.array(lfam), np.array(rfam)
    same = (tb == tg).astype(np.float32)
    return left, right, li, ri, lf, rf, same, (lfam[li] == rfam[ri]).astype(np.float32)


def flags(li, ri, lf, rf, same, family) -> np.ndarray:
    return np.concatenate([lf[li], rf[ri][:, :1], same[:, None], family[:, None]], axis=1)  # ten a pair


def char_side(items: list, fields, lead: int | None, first_sep: int, right_align: bool, width: int):
    out = np.full((len(items), width), PAD, np.int16)
    for j, it in enumerate(items):
        seq = [] if lead is None else [lead]
        for n, (f, cap) in enumerate(fields):
            seq.append(first_sep + n)
            seq.extend(it[f].encode("utf-8", "replace")[:cap])
        if right_align:
            out[j, width - len(seq):] = seq
        else:
            out[j, :len(seq)] = seq
    return out


def text_of(it: dict, left: bool) -> str:
    if left:
        null = {False: " not null", True: "", None: ""}[it["nullable"]]
        return (f"table: {it['table']} | column: {it['column']} | type: {it['type']}{null}"
                f"{' unique' if it['unique'] else ''}{' primary key' if it['primary'] else ''}"
                f"{' indexed' if it['index'] else ''} | column {it['position'] + 1} of {it['columns']}"
                f" | {it['tables']} tables")
    return f"table: {it['target']} | key: {it['key']} | type: {it['ktype']}{' primary key' if it['whole'] else ''}"


def text_side(tok, items: list, left: bool, width: int):
    """Token ids a side: the column's side right-aligned after [CLS], the
    target's left-aligned between two [SEP], so a pair's tokens lie together."""
    ids = tok([text_of(it, left) for it in items], add_special_tokens=False)["input_ids"]
    cap = width - (1 if left else 2)
    out = np.full((len(items), width), tok.pad_token_id, np.int32)
    for j, x in enumerate(ids):
        x = x[:cap]
        if left:
            seq = [tok.cls_token_id, *x]
            out[j, width - len(seq):] = seq
        else:
            seq = [tok.sep_token_id, *x, tok.sep_token_id]
            out[j, :len(seq)] = seq
    return out


# ---------------------------------------------------------------- the networks

class Char(nn.Module):
    def __init__(self, dim, layers, heads, ff, dropout=0.1, **_):
        super().__init__()
        self.emb = nn.Embedding(SEP0 + 6, dim, padding_idx=PAD)
        self.pos = nn.Embedding(LEFT_LEN + RIGHT_LEN, dim)
        layer = nn.TransformerEncoderLayer(dim, heads, ff, dropout, batch_first=True, norm_first=True, activation="gelu")
        self.enc = nn.TransformerEncoder(layer, layers, enable_nested_tensor=False)
        self.norm = nn.LayerNorm(dim)
        self.head = nn.Sequential(nn.Linear(dim + 10, dim), nn.GELU(), nn.Linear(dim, 1))

    def forward(self, left, right, extra):
        # left is right-aligned and right left-aligned; whole columns of padding at either end are cut
        keep_l = int((left != PAD).any(0).float().argmax())
        nr = int((right != PAD).any(0).sum())
        x = torch.cat([left[:, keep_l:], right[:, :nr]], 1)
        pos = torch.cat([torch.arange(keep_l, LEFT_LEN, device=x.device),
                         torch.arange(LEFT_LEN, LEFT_LEN + nr, device=x.device)])
        pad = x == PAD
        h = self.enc(self.emb(x) + self.pos(pos)[None], src_key_padding_mask=pad)
        h = self.norm(h)
        # the first real position of each row is its [CLS]
        first = (~pad).float().argmax(1)
        cls = h[torch.arange(len(x), device=x.device), first]
        return self.head(torch.cat([cls, extra], 1)).squeeze(1)


class Text(nn.Module):
    def __init__(self, pad_id: int, **_):
        super().__init__()
        from transformers import AutoModel
        self.enc = AutoModel.from_pretrained(GRANITE, revision=GRANITE_REV, dtype=torch.float32)
        self.pad_id = pad_id
        self.head = nn.Linear(self.enc.config.hidden_size, 1)

    def forward(self, left, right, extra=None):
        keep_l = int((left != self.pad_id).any(0).float().argmax())
        nr = int((right != self.pad_id).any(0).sum())
        x = torch.cat([left[:, keep_l:], right[:, :nr]], 1)
        mask = x != self.pad_id
        h = self.enc(input_ids=x, attention_mask=mask.long()).last_hidden_state
        first = mask.float().argmax(1)
        return self.head(h[torch.arange(len(x), device=x.device), first]).squeeze(1)


def tokenizer():
    from transformers import AutoTokenizer
    return AutoTokenizer.from_pretrained(GRANITE, revision=GRANITE_REV)


def build(name: str):
    cfg = CONFIGS[name]
    if cfg["kind"] == "char":
        return Char(**cfg), None
    tok = tokenizer()
    return Text(tok.pad_token_id), tok


class Pairs:
    """A set's pairs as tensors on a device: the sides once, the pairs as indices."""

    def __init__(self, name, tok, schemas, rec, rows=None, device="cuda"):
        left, right, li, ri, lf, rf, same, family = sides(schemas, rec, rows)
        if CONFIGS[name]["kind"] == "char":
            L = char_side(left, FIELDS_LEFT, CLS, SEP0, True, LEFT_LEN)
            R = char_side(right, FIELDS_RIGHT, None, SEP0 + 3, False, RIGHT_LEN)
            self.pad = PAD
        else:
            L = text_side(tok, left, True, TEXT_LEFT + 1)
            R = text_side(tok, right, False, TEXT_RIGHT + 2)
            self.pad = tok.pad_token_id
        self.L = torch.from_numpy(L.astype(np.int64)).to(device)
        self.R = torch.from_numpy(R.astype(np.int64)).to(device)
        self.li = torch.from_numpy(li.astype(np.int64)).to(device)
        self.ri = torch.from_numpy(ri.astype(np.int64)).to(device)
        self.extra = torch.from_numpy(flags(li, ri, lf, rf, same, family)).to(device)
        self.length = ((self.L != self.pad).sum(1)[self.li] + (self.R != self.pad).sum(1)[self.ri])
        self.n = len(li)

    def batch(self, idx):
        return self.L[self.li[idx]], self.R[self.ri[idx]], self.extra[idx]


CARD_LIMIT = 9 * 2 ** 30  # bytes of the card a fit or a scoring may hold; past it the job stops


def guard(what: str) -> None:
    """Stop with a clear message before the card's memory spills into the host's. On Windows
    the driver lets a process past the card's memory and everything then crawls: the first
    fit of char-s spent an hour of scoring at 11.8 GiB that way."""
    held = torch.cuda.memory_reserved()
    if held > CARD_LIMIT:
        raise SystemExit(f"stopped: {what} holds {held / 2 ** 30:.1f} GiB of the card, over the limit of "
                         f"{CARD_LIMIT / 2 ** 30:.0f} GiB; nothing is spilled into the host's memory")


def card_setup() -> None:
    # an allocation past the limit fails at once instead of spilling
    total = torch.cuda.get_device_properties(0).total_memory
    torch.cuda.set_per_process_memory_fraction(min(1.0, (CARD_LIMIT + 2 ** 29) / total))


@torch.inference_mode()
def scores_of(model, pairs: Pairs, batch: int, amp: bool = True) -> np.ndarray:
    """Every pair's probability. The pairs are taken longest first, so the first batch is the
    largest the scoring will ever hold and every later one fits the memory it left: taken
    shortest first, each new length asked the allocator for a larger block than any it had
    kept, and the blocks added up to the whole card. Half precision on the card always."""
    model.eval()
    order = torch.argsort(pairs.length, descending=True)
    out = torch.zeros(pairs.n, dtype=torch.float32, device=order.device)
    for k, i in enumerate(range(0, pairs.n, batch)):
        idx = order[i:i + batch]
        with torch.autocast("cuda", dtype=torch.bfloat16):
            out[idx] = torch.sigmoid(model(*pairs.batch(idx)).float())
        if k % 20 == 0:
            guard("scoring")
    return out.cpu().numpy()


def best_f1(y: np.ndarray, s: np.ndarray):
    """The threshold of the best F1 over the pairs, as keys.best_threshold reads it."""
    order = np.argsort(-s, kind="stable")
    ys, ss = y[order], s[order]
    tp = np.cumsum(ys)
    last = np.r_[ss[1:] != ss[:-1], True]  # a threshold takes every pair of its score
    k = np.flatnonzero(last)
    p, r = tp[k] / (k + 1), tp[k] / max(int(y.sum()), 1)
    f = 2 * p * r / np.maximum(p + r, 1e-12)
    j = int(np.argmax(f))
    return float(ss[k[j]]), float(p[j]), float(r[j]), float(f[j])


def one_target(scores, rec):
    sys.path.insert(0, str(HERE.parent / "round-1v"))
    import whole
    return whole.one_target(scores.astype(np.float64), rec)


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def save(model, d: Path, name: str, tok) -> dict:
    from safetensors.torch import save_file
    d.mkdir(parents=True, exist_ok=True)
    state = {k: v.detach().float().cpu().contiguous() for k, v in model.state_dict().items()}
    save_file(state, str(d / "model.safetensors"))
    files = ["model.safetensors"]
    if tok is not None:
        tok.save_pretrained(str(d / "tokenizer"))
        files += [f"tokenizer/{p.name}" for p in sorted((d / "tokenizer").iterdir())]
    return {"parameters": int(sum(v.numel() for v in state.values())),
            "bytes": {f: (d / f).stat().st_size for f in files}}


def load(name: str, device="cuda"):
    from safetensors.torch import load_file
    model, tok = build(name)
    model.load_state_dict(load_file(str(OUT / name / "model.safetensors")))
    return model.to(device).eval(), tok


# ---------------------------------------------------------------- commands

def fit(name: str) -> None:
    if STOP.exists():
        raise SystemExit("paused: corpus/round-1z/stop exists")
    cfg = CONFIGS[name]
    d = OUT / name
    d.mkdir(parents=True, exist_ok=True)
    torch.manual_seed(SEED)
    np.random.seed(SEED)
    t0 = time.time()
    log = lambda *a: print(f"{name} {time.time() - t0:6.0f} s:", *a, flush=True)  # noqa: E731
    data = pickle.load(open(FIRST / "train.pkl", "rb"))
    rec = data["pairs"]
    rows = np.flatnonzero(rec["sample"])
    y = torch.from_numpy(rec["label"][rows].astype(np.float32)).cuda()
    model, tok = build(name)
    model.cuda()
    train = Pairs(name, tok, data["schemas"], rec, rows)
    del data
    val = pickle.load(open(FIRST / "validation.pkl", "rb"))
    vrec = val["pairs"]
    valid = Pairs(name, tok, val["schemas"], vrec)
    yv = vrec["label"].astype(bool)
    log(f"{train.n} training rows, {int(y.sum())} positive; {valid.n} validation pairs; "
        f"{sum(p.numel() for p in model.parameters())} parameters")
    amp = True  # half precision in training for every network: the character ones too
    card_setup()
    steps = cfg["epochs"] * math.ceil(train.n / cfg["batch"])
    opt = torch.optim.AdamW(model.parameters(), lr=cfg["lr"], weight_decay=0.01)
    warm = int(0.05 * steps)
    sched = torch.optim.lr_scheduler.LambdaLR(
        opt, lambda s: (s + 1) / warm if s < warm else 0.5 * (1 + math.cos(math.pi * (s - warm) / max(steps - warm, 1))))
    gen = torch.Generator(device="cuda").manual_seed(SEED)
    loss_fn = nn.BCEWithLogitsLoss()
    history, best = [], None
    step = 0
    for epoch in range(1, cfg["epochs"] + 1):
        model.train()
        perm = torch.randperm(train.n, device="cuda", generator=gen)
        total, seen = 0.0, 0
        for i in range(0, train.n, cfg["batch"]):
            idx = perm[i:i + cfg["batch"]]
            with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                logit = model(*train.batch(idx))
            loss = loss_fn(logit.float(), y[idx])
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()
            sched.step()
            step += 1
            total += float(loss) * len(idx)
            seen += len(idx)
            if step % 50 == 0:
                guard("training")
            if step % 100 == 0 and STOP.exists():
                raise SystemExit("paused in training: nothing of this fit is kept, it starts again")
            if step % 500 == 0:
                log(f"epoch {epoch} step {step}/{steps} loss {total / seen:.4f}")
        del logit, loss
        # the epoch's weights are kept before the scoring, and the training's cached memory is given
        # back to the card first: the scoring's own blocks come on top of it otherwise
        from safetensors.torch import save_file
        save_file({k: v.detach().float().cpu().contiguous() for k, v in model.state_dict().items()}, str(d / "last.safetensors"))
        opt.zero_grad(set_to_none=True)
        torch.cuda.empty_cache()
        sv = scores_of(model, valid, 1024 if cfg["kind"] == "char" else 256)
        log(f"validation scored, the card at most {torch.cuda.max_memory_reserved() / 2 ** 30:.1f} GiB")
        kept = one_target(sv, vrec)
        th, p, r, f = best_f1(yv, kept)
        history.append({"epoch": epoch, "loss": total / seen, "threshold": th, "precision": p, "recall": r, "f1": f})
        log(json.dumps(history[-1]))
        if best is None or f > best["f1"]:
            best = history[-1]
            sizes = save(model, d, name, tok)
            np.save(d / "validation.npy", sv)
    result = {"name": name, "config": cfg, "seed": SEED, "training rows": train.n, "validation pairs": valid.n,
              "history": history, "chosen": best, **sizes, "seconds": round(time.time() - t0),
              "torch": torch.__version__, "card": torch.cuda.get_device_name(0),
              "card memory at most, bytes": int(torch.cuda.max_memory_allocated())}
    if cfg["kind"] == "text":
        result["pretrained"] = {"name": GRANITE, "revision": GRANITE_REV, "licence": "apache-2.0"}
    (d / "fit.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    log("done", json.dumps(best))


def score(name: str, src: str, dst: str) -> None:
    model, tok = load(name)
    data = pickle.load(open(src, "rb"))
    schemas = data["schemas"] if "schemas" in data else pickle.load(open(sys.argv[5], "rb"))["schemas"]
    pairs = Pairs(name, tok, schemas, data["pairs"])
    card_setup()
    t0 = time.time()
    s = scores_of(model, pairs, 1024 if CONFIGS[name]["kind"] == "char" else 256)
    Path(dst).parent.mkdir(parents=True, exist_ok=True)
    np.save(dst, s)
    print(f"{name} {src}: {pairs.n} pairs in {time.time() - t0:.0f} s on the card", flush=True)


def quantise(name: str) -> None:
    """Linear layers to 8 bits (PyTorch's dynamic quantisation), saved, and read on a part of the validation split."""
    torch.set_num_threads(8)
    # PyTorch's fast path of a transformer layer reads a linear layer's weight as a tensor, and a
    # quantised layer's is a method: the first run of char-s saved its bytes and failed at its first forward
    torch.backends.mha.set_fastpath_enabled(False)
    model, tok = load(name, "cpu")
    q = torch.ao.quantization.quantize_dynamic(model, {nn.Linear}, dtype=torch.qint8)
    d = OUT / name
    torch.save(q.state_dict(), d / "model-int8.pt")
    if CONFIGS[name]["kind"] == "text":
        from safetensors.torch import save_file
        save_file({k: v.half().contiguous() for k, v in model.state_dict().items()}, str(d / "model-fp16.safetensors"))
    val = pickle.load(open(FIRST / "validation.pkl", "rb"))
    vrec = val["pairs"]
    # every pair of the first schemas up to about 60 000 pairs: whole schemas, so one target a column holds
    n = int(np.searchsorted(np.cumsum(np.bincount(vrec["schema"])), 60000))
    rows = np.flatnonzero(vrec["schema"] <= n)
    sub = {k: v[rows] for k, v in vrec.items()}
    pairs = Pairs(name, tok, val["schemas"], sub, device="cpu")
    out = {}
    for label, m in (("float", model), ("int8", q)):
        t0 = time.time()
        s = scores_of_cpu(m, pairs, 256)
        kept = one_target(s, sub)
        th = json.load(open(d / "fit.json", encoding="utf-8"))["chosen"]["threshold"]
        tp = int(((kept >= th) & sub["label"]).sum())
        fp = int(((kept >= th) & ~sub["label"]).sum())
        fn = int(sub["label"].sum()) - tp
        out[label] = {"pairs": len(rows), "f1 at the frozen threshold": 2 * tp / max(2 * tp + fp + fn, 1),
                      "f1 at its own best": best_f1(sub["label"].astype(bool), kept)[3], "seconds": time.time() - t0}
        out[label + ", scores"] = s
    diff = float(np.abs(out.pop("float, scores") - out.pop("int8, scores")).max())
    res = {"name": name, "bytes, int8 state": (d / "model-int8.pt").stat().st_size,
           "bytes, fp16": (d / "model-fp16.safetensors").stat().st_size if (d / "model-fp16.safetensors").exists() else None,
           "validation part": out, "largest difference of a score": diff}
    (d / "quantised.json").write_text(json.dumps(res, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(res, indent=1))


@torch.no_grad()
def scores_of_cpu(model, pairs: Pairs, batch: int) -> np.ndarray:
    model.eval()
    order = torch.argsort(pairs.length)
    out = torch.zeros(pairs.n)
    for i in range(0, pairs.n, batch):
        idx = order[i:i + batch]
        out[idx] = torch.sigmoid(model(*pairs.batch(idx)).float())
    return out.numpy()


def timing(name: str, dst: str, sets: list) -> None:
    """Seconds a database on one thread: its sides made, tokenised and scored. A database
    over CAP pairs is timed on its first CAP and scaled."""
    CAP = 2000
    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    model, tok = load(name, "cpu")
    rows_out = []
    warm = False
    for src in sets:
        data = pickle.load(open(src, "rb"))
        rec = data["pairs"]
        schemas = data["schemas"]
        bounds = np.flatnonzero(np.r_[True, rec["schema"][1:] != rec["schema"][:-1], True])
        for a, b in zip(bounds[:-1], bounds[1:]):
            rows = np.arange(a, min(b, a + CAP))
            if not warm:
                scores_of_cpu(model, Pairs(name, tok, schemas, rec, rows[:256], device="cpu"), 64)
                warm = True
            t0 = time.perf_counter()
            pairs = Pairs(name, tok, schemas, rec, rows, device="cpu")
            scores_of_cpu(model, pairs, 64)
            dt = time.perf_counter() - t0
            rows_out.append({"set": Path(src).stem, "schema": int(rec["schema"][a]), "pairs": int(b - a),
                             "timed pairs": len(rows), "seconds": dt * (b - a) / len(rows)})
            print(rows_out[-1], flush=True)
    secs = np.array([r["seconds"] for r in rows_out])
    per_pair = np.array([r["seconds"] / r["pairs"] for r in rows_out])
    import platform
    res = {"name": name, "databases": len(rows_out), "timed whole": int(sum(r["pairs"] == r["timed pairs"] for r in rows_out)),
           "a database, median seconds": float(np.median(secs)), "90th percentile": float(np.percentile(secs, 90)),
           "99th percentile": float(np.percentile(secs, 99)), "a pair, a database's median seconds": float(np.median(per_pair)),
           "threads": torch.get_num_threads(), "torch": torch.__version__, "platform": platform.platform(),
           "per database": rows_out}
    Path(dst).write_text(json.dumps(res, indent=1) + "\n", encoding="utf-8")
    print({k: v for k, v in res.items() if k != "per database"})


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "fit":
        fit(sys.argv[2])
    elif cmd == "score":
        score(sys.argv[2], sys.argv[3], sys.argv[4])
    elif cmd == "quantise":
        quantise(sys.argv[2])
    else:
        timing(sys.argv[2], sys.argv[3], sys.argv[4:])

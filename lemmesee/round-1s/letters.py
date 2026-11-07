"""Round 1s: the letter trigrams in a fixed order.

Round 1b's `keys.py` and round 1p's `widen.py` keep `list(sg)[:12]` of a
stem's set of trigrams, an order Python's per-process string hashing decides.
Importing this module makes `keys.grams` return a frozenset that iterates in
sorted order, so the same line keeps the twelve smallest by code point, as
`sorted(sg)[:12]` would; nothing else about the features changes, and round
1b's and round 1p's files are left as they were.

Every process that builds features must import it: the main one by importing
this module, a pool's workers through `pool()`, which runs `apply` as each
worker starts. `LEMMESEE_GRAMS=process` in the environment, which a pool's
workers inherit, keeps the order rounds 1b to 1r had, to score their models
as they were scored.
"""

import os
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
for p in (HERE.parent / "round-1b", HERE.parent / "round-1p", HERE.parent / "round-1q", HERE.parent / "round-1r"):
    if str(p) not in sys.path:
        sys.path.append(str(p))

import keys  # noqa: E402


class Sorted(frozenset):
    """A set of trigrams that iterates in sorted order."""

    __slots__ = ()

    def __iter__(self):
        return iter(sorted(frozenset.__iter__(self)))


_original = keys.grams


def grams(s: str) -> Sorted:
    return Sorted(_original(s))


def order() -> str:
    return os.environ.get("LEMMESEE_GRAMS", "sorted")


def apply() -> None:
    """The trigrams in the order the environment asks, in this process."""
    import keys as k  # the module the features read, whatever imported it
    k.grams = grams if order() == "sorted" else _original


def is_sorted() -> bool:
    import keys as k
    return k.grams is grams


def pool(workers: int) -> ProcessPoolExecutor:
    return ProcessPoolExecutor(workers, initializer=apply)


apply()

"""Scores every method on made-up typos of word triples drawn from the 3-gram
counts (Wikipedia). Use it to tune a method; use report.py, on text the
counts have never seen, to judge it.

    python evaluate.py [samples] [seed]
"""

import random
import sys
import time

from common import load
from methods import load_all
from typos import mistype

sys.stdout.reconfigure(encoding="utf-8")


def samples(model, n, rng):
    """(context, typed keys, right word) for `n` word triples drawn by
    how often they occur."""
    triples = [(ab, c, n) for ab, nexts in model.trigram.items() for c, n in nexts.items()]
    picked = rng.choices(triples, weights=[n for _, _, n in triples], k=n)
    return [(ab, mistype(word, rng)[0], word) for ab, word, _ in picked]


def score(method, tests):
    top1 = top5 = 0
    for context, keys, want in tests:
        words = [w for w, _ in method.rank(context, keys)]
        top1 += words[:1] == [want]
        top5 += want in words[:5]
    return top1 / len(tests), top5 / len(tests)


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 500
    rng = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 1)
    model = load()
    tests = samples(model, n, rng)
    print(f"{len(tests)} lỗi giả, ví dụ: " + ", ".join(f"{' '.join(c)} {k}→{w}" for c, k, w in tests[:4]))
    for method in load_all(model):
        t = time.perf_counter()
        top1, top5 = score(method, tests)
        ms = (time.perf_counter() - t) * 1000 / len(tests)
        print(f"  {method.name:16} đúng top 1: {top1:6.1%}   có trong top 5: {top5:6.1%}   [{ms:.0f} ms/từ, Python]")


if __name__ == "__main__":
    main()

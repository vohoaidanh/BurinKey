"""Accuracy report for every method on a fixed test set.

    python report.py [tests.tsv]

Prints overall, per kind of slip and per word length, and writes every
case each method gets wrong to <tests>.<method>.wrong.tsv."""

import sys
import time
from collections import defaultdict
from pathlib import Path

from common import load, telex
from methods import load_all
from typos import KINDS

sys.stdout.reconfigure(encoding="utf-8")


def length_group(word):
    n = len(telex(word))
    return "1-3 phím" if n <= 3 else "4-5 phím" if n <= 5 else "6+ phím"


def pct(hits, total):
    return f"{hits / total:6.1%}" if total else "     -"


def main():
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent / "samples" / "giao_thong.tests.tsv"
    lines = path.read_text(encoding="utf-8").splitlines()[1:]
    tests = [line.split("\t") for line in lines if line]
    model = load()
    methods = load_all(model)

    results = {}
    for method in methods:
        t = time.perf_counter()
        guesses = [[w for w, _ in method.rank(prev, keys)] for prev, keys, _, _ in tests]
        ms = (time.perf_counter() - t) * 1000 / len(tests)
        results[method.name] = (guesses, ms)

    names = [m.name for m in methods]
    width = max(len(n) for n in names) + 2
    print(f"Tập test: {path.name}, {len(tests)} ca\n")
    print(f"{'':22}" + "".join(f"{n:>{width}}" for n in names))

    def row(label, cases, hit):
        cells = "".join(
            f"{pct(sum(hit(results[n][0][i], tests[i]) for i in cases), len(cases)):>{width}}"
            for n in names
        )
        print(f"{label:22}{cells}")

    every = range(len(tests))
    row("Đúng top 1", every, lambda g, t: g[:1] == [t[2]])
    row("Có trong top 5", every, lambda g, t: t[2] in g[:5])

    for title, key in [("Theo loại lỗi (top 1)", lambda t: KINDS[t[3]]), ("Theo độ dài từ (top 1)", lambda t: length_group(t[2]))]:
        groups = defaultdict(list)
        for i, t in enumerate(tests):
            groups[key(t)].append(i)
        print(f"\n{title}")
        for label in sorted(groups):
            row(f"  {label} ({len(groups[label])})", groups[label], lambda g, t: g[:1] == [t[2]])

    print(f"\n{'Thời gian / từ (Python)':22}" + "".join(f"{results[n][1]:>{width - 3}.1f} ms" for n in names))

    for n in names:
        guesses = results[n][0]
        wrong = [(t, g[0] if g else "") for t, g in zip(tests, guesses) if g[:1] != [t[2]]]
        out = path.with_name(f"{path.stem}.{n}.wrong.tsv")
        out.write_text(
            "prev\ttyped\twant\tkind\tguess\n" + "".join("\t".join(t) + f"\t{g}\n" for t, g in wrong),
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()

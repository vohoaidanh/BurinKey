"""How much accuracy keyboard_trigram keeps as rare 3-grams are dropped,
to choose a size for the app.

    python prune.py [tests.tsv]

For each minimum count: triples kept, estimated packed size, and top-1
accuracy on the paragraph test set."""

import sys
from pathlib import Path

from common import load
from methods.keyboard_trigram import Method

sys.stdout.reconfigure(encoding="utf-8")

BYTES_PER_TRIPLE = 7  # three 13-bit syllable ids + a 1-byte log-probability


def main():
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent / "samples" / "giao_thong.tests.tsv"
    tests = [line.split("\t") for line in path.read_text(encoding="utf-8").splitlines()[1:] if line]
    model = load()
    full = model.trigram
    method = Method(model)

    print(f"Tập test: {path.name}, {len(tests)} ca\n")
    print(f"{'đếm tối thiểu':>14} {'số bộ giữ lại':>14} {'dung lượng':>11} {'đúng top 1':>11}")
    for min_count in (3, 10, 30, 100, 300, 1000):
        pruned = {}
        for pair, nexts in full.items():
            kept = {w: n for w, n in nexts.items() if n >= min_count}
            if kept:
                pruned[pair] = kept
        model._trigram = pruned
        triples = sum(len(v) for v in pruned.values())
        right = sum(
            method.rank(tuple(c.split()), keys)[0][0] == want for c, keys, want, _ in tests
        )
        size = triples * BYTES_PER_TRIPLE / 1e6
        print(f"{min_count:>14} {triples:>14,} {size:>8.1f} MB {right / len(tests):>11.1%}", flush=True)


if __name__ == "__main__":
    main()

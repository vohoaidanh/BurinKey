"""Grid search for keyboard_bigram's numbers on Wikipedia-drawn typos
(never on the paragraph test sets, which stay for judging).

    python tune.py [samples] [seed]
"""

import itertools
import random
import sys

from common import load
from evaluate import samples, score
from keyboard import Costs
from methods.keyboard_bigram import Method

sys.stdout.reconfigure(encoding="utf-8")


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 300
    rng = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 11)
    model = load()
    tests = samples(model, n, rng)
    method = Method(model)
    results = []
    for per_unit, near, edit in itertools.product([3.0, 4.0, 5.0, 6.0], [0.5, 0.7, 0.9], [0.5, 0.6, 0.8]):
        method.per_unit = per_unit
        method.costs = Costs(near=near, far=1.0, swap=0.5, add=edit, drop=edit)
        top1, top5 = score(method, tests)
        results.append((top1, top5, per_unit, near, edit))
        print(f"per_unit={per_unit:<4} near={near:<4} add/drop={edit:<4} top1={top1:6.1%} top5={top5:6.1%}", flush=True)
    best = max(results)
    print(f"\ntốt nhất: per_unit={best[2]} near={best[3]} add/drop={best[4]}  top1={best[0]:.1%}")


if __name__ == "__main__":
    main()

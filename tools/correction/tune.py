"""Grid search for a method's numbers on Wikipedia-drawn typos (never on
the paragraph test sets, which stay for judging).

    python tune.py keyboard_bigram [samples] [seed]
    python tune.py keyboard_trigram [samples] [seed]

Prints every setting and the best one; copy the best into the method's
defaults by hand."""

import itertools
import random
import sys
from importlib import import_module

from common import load
from evaluate import samples, score
from keyboard import Costs

sys.stdout.reconfigure(encoding="utf-8")


def costs(near, edit):
    return Costs(near=near, far=1.0, swap=0.5, add=edit, drop=edit)


# For each method: names of the knobs, their values to try, and how to
# set them on a method object.
GRIDS = {
    "keyboard_bigram": (
        ["per_unit", "near", "add/drop"],
        [[3.0, 4.0, 5.0, 6.0], [0.5, 0.7, 0.9], [0.5, 0.6, 0.8]],
        lambda m, per_unit, near, edit: (setattr(m, "per_unit", per_unit), setattr(m, "costs", costs(near, edit))),
    ),
    "keyboard_trigram": (
        ["per_unit", "mix"],
        [[4.0, 5.0, 6.0], [0.3, 0.5, 0.7, 0.9]],
        lambda m, per_unit, mix: (setattr(m, "per_unit", per_unit), setattr(m, "mix", mix)),
    ),
}


def main():
    name = sys.argv[1] if len(sys.argv) > 1 else "keyboard_trigram"
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    rng = random.Random(int(sys.argv[3]) if len(sys.argv) > 3 else 11)
    labels, values, apply = GRIDS[name]
    model = load()
    tests = samples(model, n, rng)
    method = import_module(f"methods.{name}").Method(model)
    results = []
    for setting in itertools.product(*values):
        apply(method, *setting)
        top1, top5 = score(method, tests)
        results.append((top1, top5, setting))
        shown = "  ".join(f"{k}={v:<4}" for k, v in zip(labels, setting))
        print(f"{shown}  top1={top1:6.1%} top5={top5:6.1%}", flush=True)
    top1, _, setting = max(results)
    print(f"\ntốt nhất: " + "  ".join(f"{k}={v}" for k, v in zip(labels, setting)) + f"  top1={top1:.1%}")


if __name__ == "__main__":
    main()

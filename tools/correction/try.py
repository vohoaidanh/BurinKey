"""Shows what each method suggests for a few typed words.

    python try.py                       # the built-in examples
    python try.py thao truowfn          # previous word, typed keys
    python try.py - hock                # no previous word
    python try.py -i                    # type whole sentences in Telex
"""

import sys
import time

from common import load
from methods import load_all

sys.stdout.reconfigure(encoding="utf-8")

EXAMPLES = [
    ("thao", "truowfn", "trường"),
    ("thao", "trowowfn", "trường"),
    ("nhà", "truognwf", "trường"),
    ("đi", "hock", "học"),
    ("đi", "hocjj", "học"),
    ("thành", "phoos", "phố"),
    ("phố", "haie", "hải"),
    ("đi", "haie", "hái"),
    (None, "truongwg", "trường"),
]


def interactive(methods):
    """Each word of a typed line is corrected, using the previous corrected
    word as context."""
    print("Gõ một câu bằng phím Telex (vd: ddi hock truognwf), Enter để thoát.")
    while line := input("\n> ").strip().lower():
        for method in methods:
            prev, out = None, []
            for keys in line.split():
                ranked = method.rank(prev, keys)
                prev = ranked[0][0] if ranked else keys
                out.append(prev)
            print(f"  {method.name:11} {' '.join(out)}")


def main():
    model = load()
    methods = load_all(model)
    if sys.argv[1:] == ["-i"]:
        return interactive(methods)
    if len(sys.argv) == 3:
        prev = None if sys.argv[1] == "-" else sys.argv[1]
        cases = [(prev, sys.argv[2], None)]
    else:
        cases = EXAMPLES
    for prev, keys, want in cases:
        print(f"\n{prev or '·'} + {keys}" + (f"   (đúng: {want})" if want else ""))
        for method in methods:
            t = time.perf_counter()
            ranked = method.rank(prev, keys)
            ms = (time.perf_counter() - t) * 1000
            words = [w for w, _ in ranked]
            mark = ""
            if want:
                mark = "✓" if words[0] == want else f"✗ (hạng {words.index(want) + 1})" if want in words else "✗ (ngoài top 10)"
            print(f"  {method.name:11} {mark:18} {', '.join(words[:6])}   [{ms:.0f} ms]")


if __name__ == "__main__":
    main()

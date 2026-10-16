"""Shows what each method suggests for a few typed words.

    python try.py                       # the built-in examples
    python try.py giao thông cua        # words before, then the typed keys
    python try.py hock                  # no words before
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
    ("thành phố", "haie", "hải"),
    ("đi", "haie", "hái"),
    ("", "truongwg", "trường"),
    ("giao thông", "cua", "của"),
    ("giao thông", "a", "an"),
    ("đô thị", "taih", "tại"),
]


def interactive(methods):
    """Each word of a typed line is corrected, using the two words before
    it (as corrected) as context."""
    print("Gõ một câu bằng phím Telex (vd: ddi hock truognwf), Enter để thoát.")
    while line := input("\n> ").strip().lower():
        for method in methods:
            out = []
            for keys in line.split():
                ranked = method.rank(tuple(out[-2:]), keys)
                out.append(ranked[0][0] if ranked else keys)
            print(f"  {method.name:16} {' '.join(out)}")


def main():
    model = load()
    methods = load_all(model)
    args = sys.argv[1:]
    if args == ["-i"]:
        return interactive(methods)
    cases = [(" ".join(args[:-1]), args[-1], None)] if args else EXAMPLES
    for context, keys, want in cases:
        print(f"\n{context or '·'} + {keys}" + (f"   (đúng: {want})" if want else ""))
        for method in methods:
            t = time.perf_counter()
            ranked = method.rank(tuple(context.split()), keys)
            ms = (time.perf_counter() - t) * 1000
            words = [w for w, _ in ranked]
            mark = ""
            if want:
                mark = "✓" if words[0] == want else f"✗ (hạng {words.index(want) + 1})" if want in words else "✗ (ngoài top 10)"
            print(f"  {method.name:16} {mark:18} {', '.join(words[:6])}   [{ms:.0f} ms]")


if __name__ == "__main__":
    main()

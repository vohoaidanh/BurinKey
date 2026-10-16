"""Turns a paragraph into a fixed typo test set.

    python make_tests.py [text file] [seed]

Every known syllable after the first becomes one test: the word before
it, the word typed with one slip, the right word, and the kind of slip.
Written next to the text as <name>.tests.tsv so all methods are judged on
exactly the same cases."""

import random
import re
import sys
import unicodedata
from pathlib import Path

from common import load
from typos import mistype

sys.stdout.reconfigure(encoding="utf-8")

SAMPLES = Path(__file__).parent / "samples"


def syllables(text):
    return re.findall(r"[^\W\d_]+", unicodedata.normalize("NFC", text.lower()))


def main():
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else SAMPLES / "giao_thong.txt"
    rng = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 7)
    known = set(load().syllables)
    words = syllables(path.read_text(encoding="utf-8"))

    rows = []
    for prev, word in zip(words, words[1:]):
        if word in known:
            keys, kind = mistype(word, rng)
            rows.append(f"{prev}\t{keys}\t{word}\t{kind}")
    out = path.with_suffix(".tests.tsv")
    out.write_text("prev\ttyped\twant\tkind\n" + "\n".join(rows) + "\n", encoding="utf-8")
    skipped = len(words) - 1 - len(rows)
    print(f"{out.name}: {len(rows)} ca test ({skipped} từ bỏ qua vì không có trong danh sách âm tiết)")


if __name__ == "__main__":
    main()

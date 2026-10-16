"""Builds data/model.pkl from
  data/words.txt              undertheseanlp dictionary (GPL-3.0)
  data/vi_2gram_word.parquet  wikilangs/vi 2-gram counts (MIT, from Wikipedia)

Keeps only Vietnamese syllables, merges case, and drops rare pairs."""

import json
import pickle
import re
import sys
import unicodedata
from collections import Counter, defaultdict

import pyarrow.parquet as pq

from common import DATA, MODEL, TONE_KEYS

sys.stdout.reconfigure(encoding="utf-8")

MIN_UNIGRAM = 20  # a syllable must appear this often in Wikipedia
MIN_PAIR = 3      # a pair must appear this often

# Rough shape check, enough for experiments (the app will use spelling.rs).
SHAPE = re.compile(
    r"^(ngh|ng|nh|ch|gh|gi|kh|ph|qu|th|tr|[bcdđghklmnprstvx])?"
    r"([aeiouy]{1,3})"
    r"(ch|ng|nh|[cmnpt])?$"
)


def is_syllable(word):
    nfd = unicodedata.normalize("NFD", word)
    tones = [c for c in nfd if c in TONE_KEYS]
    base = "".join(c for c in nfd if unicodedata.category(c) != "Mn")
    base = unicodedata.normalize("NFC", base)
    m = SHAPE.match(base)
    if not m or len(tones) > 1:
        return False
    stop = m.group(3) in ("c", "ch", "p", "t")
    return not stop or (tones and tones[0] in ("́", "̣"))


def dictionary_syllables():
    found = set()
    for line in open(DATA / "words.txt", encoding="utf-8"):
        text = unicodedata.normalize("NFC", json.loads(line)["text"].lower())
        found.update(s for s in re.split(r"[\s\-]+", text) if is_syllable(s))
    return found


def main():
    known = dictionary_syllables()
    print(f"âm tiết trong từ điển: {len(known)}")

    table = pq.read_table(DATA / "vi_2gram_word.parquet", columns=["ngram", "frequency"])
    unigram, pairs = Counter(), Counter()
    for ngram, freq in zip(table.column("ngram").to_pylist(), table.column("frequency").to_pylist()):
        a, b = (unicodedata.normalize("NFC", w.lower()) for w in json.loads(ngram))
        unigram[b] += freq
        if a in known and b in known:
            pairs[a, b] += freq

    syllables = sorted(s for s in known if unigram[s] >= MIN_UNIGRAM)
    keep = set(syllables)
    bigram = defaultdict(dict)
    for (a, b), n in pairs.items():
        if n >= MIN_PAIR and a in keep and b in keep:
            bigram[a][b] = n
    unigram = {s: unigram[s] for s in syllables}

    entries = sum(len(v) for v in bigram.values())
    print(f"âm tiết giữ lại: {len(syllables)}")
    print(f"cặp 2-gram giữ lại: {entries}  (~{entries * 5 / 1e6:.1f} MB nếu 5 byte/cặp)")
    with open(MODEL, "wb") as f:
        pickle.dump((syllables, unigram, dict(bigram)), f)
    print(f"đã ghi {MODEL}")


if __name__ == "__main__":
    main()

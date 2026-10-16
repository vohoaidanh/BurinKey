"""Shared data for the correction experiments: the syllable list, Telex keys
of each syllable, and the 2-gram counts. Built once by prepare.py into
data/model.pkl; every method loads it from there."""

import pickle
import unicodedata
from pathlib import Path

DATA = Path(__file__).parent / "data"
MODEL = DATA / "model.pkl"

TONE_KEYS = {"́": "s", "̀": "f", "̉": "r", "̃": "x", "̣": "j"}
CIRCUMFLEX, BREVE, HORN = "̂", "̆", "̛"
LETTERS = "abcdefghijklmnopqrstuvwxyz"


def telex(syllable):
    """The plain Telex keys for a syllable: letters, then the mark keys,
    then the tone key. trường -> truongwf, cân -> cana, đi -> did."""
    letters, marks, tone = [], [], ""
    for ch in unicodedata.normalize("NFD", syllable):
        if ch in TONE_KEYS:
            tone = TONE_KEYS[ch]
        elif ch == CIRCUMFLEX:
            marks.append(letters[-1])
        elif ch in (BREVE, HORN):
            if "w" not in marks:
                marks.append("w")
        elif ch == "đ":
            letters.append("d")
            marks.append("d")
        else:
            letters.append(ch)
    return "".join(letters) + "".join(marks) + tone


def letter_count(syllable):
    """How many of the syllable's Telex keys are letters; the keys after
    them are marks."""
    return sum(unicodedata.category(c) != "Mn" for c in unicodedata.normalize("NFD", syllable))


def bag(keys):
    """Key counts, a-z, ignoring order."""
    counts = [0] * 26
    for k in keys:
        i = ord(k) - 97
        if 0 <= i < 26:
            counts[i] += 1
    return tuple(counts)


def bag_distance(a, b):
    return sum(abs(x - y) for x, y in zip(a, b))


class Model:
    """syllables: list[str]; unigram: dict[str, int];
    bigram: dict[str, dict[str, int]] (prev -> next -> count);
    keys/bags: Telex keys and key bags of each syllable."""

    def __init__(self, syllables, unigram, bigram):
        self.syllables = syllables
        self.unigram = unigram
        self.bigram = bigram
        self.total = sum(unigram.values())
        self.keys = {s: telex(s) for s in syllables}
        self.bags = [(s, bag(self.keys[s])) for s in syllables]
        self.prev_total = {p: sum(n.values()) for p, n in bigram.items()}

    def p_unigram(self, word):
        return (self.unigram.get(word, 0) + 1) / (self.total + len(self.syllables))

    def p_bigram(self, prev, word, mix=0.8):
        """P(word | prev), interpolated with the unigram so unseen pairs
        never score zero."""
        nexts = self.bigram.get(prev)
        if not nexts:
            return self.p_unigram(word)
        seen = nexts.get(word, 0) / self.prev_total[prev]
        return mix * seen + (1 - mix) * self.p_unigram(word)


def load():
    with open(MODEL, "rb") as f:
        syllables, unigram, bigram = pickle.load(f)
    return Model(syllables, unigram, bigram)

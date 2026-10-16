"""Shared data for the correction experiments: the syllable list, Telex keys
of each syllable, and the 2-gram counts. Built once by prepare.py into
data/model.pkl; every method loads it from there."""

import pickle
import unicodedata
from pathlib import Path

DATA = Path(__file__).parent / "data"
MODEL = DATA / "model.pkl"
TRIGRAM = DATA / "trigram.pkl"

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
    trigram: dict[(prev2, prev), dict[str, int]], loaded on first use;
    keys/bags: Telex keys and key bags of each syllable.

    A context is a tuple of the words before, most recent last; it may be
    empty (start of a sentence)."""

    def __init__(self, syllables, unigram, bigram):
        self.syllables = syllables
        self.unigram = unigram
        self.bigram = bigram
        self.total = sum(unigram.values())
        self.keys = {s: telex(s) for s in syllables}
        self.bags = [(s, bag(self.keys[s])) for s in syllables]
        self.prev_total = {p: sum(n.values()) for p, n in bigram.items()}
        self._trigram = None

    @property
    def trigram(self):
        if self._trigram is None:
            with open(TRIGRAM, "rb") as f:
                self._trigram = pickle.load(f)
        return self._trigram

    def pair_count(self, a, b):
        """How often `a b` occurs: the 3-gram denominator. Taken from the
        2-gram counts, so it stays right when rare triples are dropped."""
        return self.bigram.get(a, {}).get(b, 0)

    def p_unigram(self, word):
        return (self.unigram.get(word, 0) + 1) / (self.total + len(self.syllables))

    def p_bigram(self, context, word, mix=0.8):
        """P(word | last word), interpolated with the unigram so unseen
        pairs never score zero."""
        nexts = self.bigram.get(context[-1]) if context else None
        if not nexts:
            return self.p_unigram(word)
        seen = nexts.get(word, 0) / self.prev_total[context[-1]]
        return mix * seen + (1 - mix) * self.p_unigram(word)

    def p_trigram(self, context, word, mix=0.7, mix2=0.8):
        """P(word | last two words), backing off to the 2-gram."""
        lower = self.p_bigram(context, word, mix2)
        nexts = self.trigram.get(tuple(context[-2:])) if len(context) >= 2 else None
        total = self.pair_count(*context[-2:]) if nexts else 0
        if not total:
            return lower
        seen = min(nexts.get(word, 0) / total, 1.0)
        return mix * seen + (1 - mix) * lower

    def likely_next(self, context, count, trigram=False):
        """The words that most often follow the last word (and, with
        `trigram`, the last two words): up to `count` from each table."""
        tables = [self.bigram.get(context[-1], {})] if context else []
        if trigram and len(context) >= 2:
            tables.append(self.trigram.get(tuple(context[-2:]), {}))
        found = []
        for nexts in tables:
            found += sorted(nexts, key=nexts.get, reverse=True)[:count]
        return list(dict.fromkeys(found))


def load():
    with open(MODEL, "rb") as f:
        syllables, unigram, bigram = pickle.load(f)
    return Model(syllables, unigram, bigram)

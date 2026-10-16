"""Context + keys: score = log P(word | prev) - per_key * (keys off).

Candidates come from both sides: the closest syllables by key bag, and the
words that most often follow `prev`, so a badly garbled word can still be
found through its context."""

import math

from common import bag, bag_distance


class Method:
    name = "bag_bigram"

    def __init__(self, model, per_key=2.0, by_keys=50, by_context=50):
        self.model = model
        self.per_key = per_key
        self.by_keys = by_keys
        self.by_context = by_context

    def rank(self, prev, keys, top=10):
        m = self.model
        typed = bag(keys)
        dist = {s: bag_distance(typed, b) for s, b in m.bags}

        candidates = set(sorted(dist, key=dist.get)[: self.by_keys])
        nexts = m.bigram.get(prev, {})
        candidates.update(sorted(nexts, key=nexts.get, reverse=True)[: self.by_context])

        scored = [
            (s, math.log(m.p_bigram(prev, s)) - self.per_key * dist[s]) for s in candidates
        ]
        scored.sort(key=lambda x: -x[1])
        return scored[:top]

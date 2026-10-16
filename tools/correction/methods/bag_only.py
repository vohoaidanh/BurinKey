"""Baseline: the syllables whose Telex keys are closest to the typed keys
(as an unordered bag), ties broken by how common the syllable is.
No context."""

import math

from common import bag, bag_distance


class Method:
    name = "bag_only"

    def __init__(self, model, per_key=2.0):
        self.model = model
        self.per_key = per_key

    def rank(self, prev, keys, top=10):
        typed = bag(keys)
        scored = [
            (s, -self.per_key * bag_distance(typed, b) + math.log(self.model.p_unigram(s)))
            for s, b in self.model.bags
        ]
        scored.sort(key=lambda x: -x[1])
        return scored[:top]

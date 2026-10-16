"""Context + keyboard-aware likeness:

    score = log P(word | prev) - per_unit * keyboard_distance(typed, word)

The distance follows key order and knows the QWERTY layout, so a slip to
a neighbouring key or two swapped keys cost less than a far-off key.
Telex lets the mark keys come in any order, so each candidate is compared
in every order of its mark keys and the closest one counts.

Candidates are found as in bag_bigram (closest key bags + likely next
words), then re-scored."""

import math
from itertools import permutations

from common import bag, bag_distance, letter_count
from keyboard import Costs, distance


class Method:
    name = "keyboard_bigram"

    # Defaults picked by tune.py on Wikipedia-drawn typos.
    def __init__(self, model, per_unit=5.0, costs=None, by_keys=60, by_context=50):
        self.model = model
        self.per_unit = per_unit
        self.costs = costs or Costs(near=0.5, far=1.0, swap=0.5, add=0.5, drop=0.5)
        self.by_keys = by_keys
        self.by_context = by_context
        # Every spelling of each syllable: letters, then its marks in any order.
        self.forms = {}
        for s in model.syllables:
            keys = model.keys[s]
            n = letter_count(s)
            self.forms[s] = {keys[:n] + "".join(p) for p in permutations(keys[n:])}

    def likeness(self, typed, word, limit):
        return min(distance(typed, form, self.costs, limit) for form in self.forms[word])

    # The language-model side; keyboard_trigram overrides these two.
    def p_context(self, context, word):
        return self.model.p_bigram(context, word)

    def likely_next(self, context):
        return self.model.likely_next(context, self.by_context)

    def rank(self, context, keys, top=10):
        typed = bag(keys)
        by_bag = sorted(self.model.bags, key=lambda sb: bag_distance(typed, sb[1]))
        candidates = {s for s, _ in by_bag[: self.by_keys]}
        candidates.update(self.likely_next(context))

        scored = []
        for s in candidates:
            d = self.likeness(keys, s, limit=4.0)
            scored.append((s, math.log(self.p_context(context, s)) - self.per_unit * d))
        scored.sort(key=lambda x: -x[1])
        return scored[:top]

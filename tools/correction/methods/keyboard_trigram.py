"""keyboard_bigram with a 3-gram: the context is the last two words, so
after a whole compound ("giao thông") the next word is not mistaken for
the second half of a compound starting with its last syllable
("thông qua"). Backs off to the 2-gram, then to word frequency."""

from methods import keyboard_bigram


class Method(keyboard_bigram.Method):
    name = "keyboard_trigram"

    # mix: how much to trust the 3-gram over the 2-gram; picked by tune.py.
    def __init__(self, model, mix=0.7, **kwargs):
        super().__init__(model, **kwargs)
        self.mix = mix

    def p_context(self, context, word):
        return self.model.p_trigram(context, word, mix=self.mix)

    def likely_next(self, context):
        return self.model.likely_next(context, self.by_context, trigram=True)

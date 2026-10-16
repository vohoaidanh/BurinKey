"""Makes believable Telex typing slips, for testing."""

from common import letter_count, telex
from keyboard import NEIGHBOURS

KINDS = {
    "swap": "đảo 2 phím",
    "drop": "thiếu 1 phím",
    "add": "thừa phím kề",
    "near": "nhầm phím kề",
}


def typed_keys(word, rng):
    """Telex keys with the mark keys (which follow the letters) shuffled,
    as people type them in any order."""
    keys = telex(word)
    n = letter_count(word)
    marks = list(keys[n:])
    rng.shuffle(marks)
    return keys[:n] + "".join(marks)


def slip(keys, rng):
    """One random slip. Returns (slipped keys, kind)."""
    kind = rng.choice(list(KINDS))
    if len(keys) < 2 and kind in ("swap", "drop"):
        kind = "near"
    i = rng.randrange(len(keys))
    if kind == "swap":
        i = min(i, len(keys) - 2)
        return keys[:i] + keys[i + 1] + keys[i] + keys[i + 2 :], kind
    if kind == "drop":
        return keys[:i] + keys[i + 1 :], kind
    near = sorted(NEIGHBOURS.get(keys[i], {keys[i]}))
    if kind == "add":
        return keys[: i + 1] + rng.choice(near) + keys[i + 1 :], kind
    return keys[:i] + rng.choice(near) + keys[i + 1 :], kind


def mistype(word, rng):
    """(typed keys, kind) for `word` typed with one slip. Tries again when
    the slip still spells the word, e.g. two mark keys swapped."""
    right, n = telex(word), letter_count(word)
    while True:
        keys, kind = slip(typed_keys(word, rng), rng)
        if keys[:n] != right[:n] or sorted(keys[n:]) != sorted(right[n:]):
            return keys, kind

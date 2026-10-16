"""QWERTY layout facts and a typing-aware edit distance."""

ROWS = ["qwertyuiop", "asdfghjkl", "zxcvbnm"]
# Rows are staggered: a key touches its left/right keys, the two keys above
# it (same index and one to the right) and the two below (one to the left
# and same index).
NEIGHBOURS = {}
for r, row in enumerate(ROWS):
    for c, key in enumerate(row):
        near = {row[c - 1] if c else None, row[c + 1] if c + 1 < len(row) else None}
        if r > 0:
            near.update(ROWS[r - 1][c : c + 2])
        if r + 1 < len(ROWS):
            near.update(ROWS[r + 1][max(c - 1, 0) : c + 1])
        near.discard(None)
        near.discard(key)
        NEIGHBOURS[key] = frozenset(near)


class Costs:
    def __init__(self, near=0.5, far=1.0, swap=0.5, add=0.8, drop=0.8):
        self.near, self.far, self.swap, self.add, self.drop = near, far, swap, add, drop

    def sub(self, a, b):
        if a == b:
            return 0.0
        return self.near if b in NEIGHBOURS.get(a, ()) else self.far


def distance(typed, target, costs, limit=float("inf")):
    """Weighted Damerau (optimal string alignment) distance from `target`
    (what should have been typed) to `typed`. Gives up early, returning
    `limit`, once every path costs more than `limit`."""
    n, m = len(target), len(typed)
    prev2 = None
    prev = [j * costs.add for j in range(m + 1)]
    for i in range(1, n + 1):
        row = [i * costs.drop] + [0.0] * m
        for j in range(1, m + 1):
            best = min(
                prev[j] + costs.drop,
                row[j - 1] + costs.add,
                prev[j - 1] + costs.sub(target[i - 1], typed[j - 1]),
            )
            if (
                prev2 is not None
                and j > 1
                and target[i - 1] == typed[j - 2]
                and target[i - 2] == typed[j - 1]
            ):
                best = min(best, prev2[j - 2] + costs.swap)
            row[j] = best
        if min(row) > limit:
            return limit
        prev2, prev = prev, row
    return prev[m]

"""Correction methods. Each module defines `Method(model)` with

    name: str
    rank(prev: str | None, keys: str) -> list[tuple[str, float]]

returning candidates best first, scores higher = better. To try a new
idea, add a module here and list it in METHODS."""

from importlib import import_module

METHODS = ["bag_only", "bag_bigram", "keyboard_bigram"]


def load_all(model, names=None):
    return [import_module(f"methods.{n}").Method(model) for n in names or METHODS]

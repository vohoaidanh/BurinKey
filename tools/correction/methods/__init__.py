"""Correction methods. Each module defines `Method(model)` with

    name: str
    rank(context: tuple[str, ...], keys: str) -> list[tuple[str, float]]

where `context` is the words before, most recent last (empty at the
start of a sentence), returning candidates best first, higher = better.
To try a new idea, add a module here and list it in METHODS."""

from importlib import import_module

METHODS = ["bag_only", "bag_bigram", "keyboard_bigram", "keyboard_trigram"]


def load_all(model, names=None):
    return [import_module(f"methods.{n}").Method(model) for n in names or METHODS]

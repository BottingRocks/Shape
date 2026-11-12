"""Session extraction from explicit initializer source."""

from typing import Any, Dict

from ._native import operation


def from_initializer(init: str) -> Dict[str, Any]:
    return operation("session.from_initializer", {"init": init})

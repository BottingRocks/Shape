"""``shape.cleanup``: cleanup of emitted thread code."""

from typing import Any, Dict

from ._native import operation as _call

__all__ = ["thread"]


def thread(source: str) -> Dict[str, Any]:
    """Clean one emitted thread: ``{"code", "cleanup": {counters}}``."""
    return _call("cleanup.thread", {"source": source})

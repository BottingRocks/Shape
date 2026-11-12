"""``shape.naming``: naming passes over traced program JavaScript."""

from typing import Any, Dict, Optional

from . import _naming_corpus as corpus
from ._native import JsonDoc, text_document
from ._native import operation as _call

__all__ = ["threads", "collectors", "plan", "inventory"]


def threads(
    source: str, rename: bool = False, labels: Optional[JsonDoc] = None
) -> Dict[str, Any]:
    """Name threads: ``{"source", "report"}``; ``labels`` is a label reference document."""
    return _call(
        "naming.threads",
        {"source": source, "rename": rename, "labels": text_document(labels, "labels")},
    )


def collectors(source: str, rename: bool = False) -> Dict[str, Any]:
    """Identify collector functions: ``{"source", "report"}``."""
    return _call("naming.collectors", {"source": source, "rename": rename})


def plan(source: str) -> Dict[str, Any]:
    """The semantic-name plan."""
    return _call("naming.plan", {"source": source})


def inventory(source: str) -> Dict[str, Any]:
    """``{"function_index", "role_graph"}``."""
    return _call("naming.inventory", {"source": source})


__all__ += ["corpus"]

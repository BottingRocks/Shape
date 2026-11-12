"""``shape.ops``: the operation (handler) catalogue of a capture."""

from typing import Any, Dict

from . import _classify as classify
from ._native import operation as _call

__all__ = ["list", "show"]


def list(source: str) -> Dict[str, Any]:  # noqa: A001 - mirrors `shape ops list`
    """Every operation the capture's VM defines."""
    return _call("ops.list", {"source": source})


def show(source: str, op: int) -> Dict[str, Any]:
    """One operation by its key."""
    return _call("ops.show", {"source": source, "op": op})


__all__ += ["classify"]

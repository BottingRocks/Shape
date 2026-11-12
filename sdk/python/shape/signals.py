"""``shape.signals``: signal analysis of traced program JavaScript."""

from typing import Any, Dict, Optional

from ._native import operation as _call

__all__ = ["schema", "show", "inputs", "contracts"]


def schema(
    source: str,
    name: Optional[str] = None,
    layouts_only: bool = False,
    finder_roles: bool = True,
) -> Dict[str, Any]:
    """The signal schema report; ``name`` sets the report's seed name."""
    return _call(
        "signals.schema",
        {
            "source": source,
            "name": name,
            "layouts_only": layouts_only,
            "finder_roles": finder_roles,
        },
    )


def show(source: str) -> Dict[str, Any]:
    """Extracted program properties."""
    return _call("signals.show", {"source": source})


def inputs(source: str) -> Dict[str, Any]:
    """Program inputs with context."""
    return _call("signals.inputs", {"source": source})


def contracts(source: str) -> Dict[str, Any]:
    """Signal contracts."""
    return _call("signals.contracts", {"source": source})


def catalog() -> list:
    """Static canonical signal cards, independent of captured source."""
    return _call("signals.catalog", {})


def describe(name: str) -> Dict[str, Any]:
    return _call("signals.describe", {"name": name})


def compact_schema(source: str, *, direct: bool = False) -> Dict[str, Any]:
    """Descriptive source export, not an executable decode artifact."""
    if not isinstance(direct, bool):
        raise TypeError("direct must be a bool")
    return _call("signals.compact_schema", {"source": source, "direct": direct})


__all__ += ["catalog", "describe", "compact_schema"]

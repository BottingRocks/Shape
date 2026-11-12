"""Naming corpus analysis and comparison from explicit named traced sources."""

from typing import Any, Dict, List, Sequence, TypedDict

from ._native import operation


class Source(TypedDict):
    name: str
    source: str


def analyze(sources: Sequence[Source]) -> List[Dict[str, Any]]:
    """Analyze already-traced programs; names are nonempty unique caller IDs."""
    return operation("naming.corpus.analyze", {"sources": list(sources)})


def compare(
    baseline: Sequence[Source], current: Sequence[Source]
) -> List[Dict[str, Any]]:
    """Reconstruct plans from supplied source arrays, then compare role/name sets."""
    return operation(
        "naming.corpus.compare", {"baseline": list(baseline), "current": list(current)}
    )

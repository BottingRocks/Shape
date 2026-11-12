"""Explicit source/kernel corpus tracing and caller-supplied comparisons."""

import os
from typing import Any, Dict, List, Optional, Sequence, Union

from ._native import JsonDoc, operation, text_document
from .cache import CachePolicy
from .cache import request as cache_request
from .kernel import LoadedKernel

Path = Union[str, os.PathLike]


def load(
    source: str, *, name: Optional[str] = None, cache: Optional[CachePolicy] = None
) -> Dict[str, Any]:
    """Load or compile source under explicit disk policy, returning a kernel."""
    return operation(
        "trace.corpus.load",
        {"source": source, "name": name, "cache": cache_request(cache)},
    )


def source(
    source: str,
    *,
    name: Optional[str] = None,
    cache: Optional[CachePolicy] = None,
    steps: int = 1_000_000,
    jobs: int = 1,
    limit: Optional[int] = None,
    cleanup: bool = False,
) -> List[Dict[str, Any]]:
    return operation(
        "trace.corpus.source",
        {
            "source": source,
            "name": name,
            "cache": cache_request(cache),
            "steps": steps,
            "jobs": jobs,
            "limit": limit,
            "cleanup": cleanup,
        },
    )


def reachable(
    artifact: Union[LoadedKernel, JsonDoc],
    *,
    steps: int = 1_000_000,
    jobs: int = 1,
    limit: Optional[int] = None,
    cleanup: bool = False,
) -> List[Dict[str, Any]]:
    """Walk roots/created functions; inspect each stop fact (limit may truncate)."""
    fields: Dict[str, Any] = {
        "steps": steps,
        "jobs": jobs,
        "limit": limit,
        "cleanup": cleanup,
    }
    if isinstance(artifact, LoadedKernel):
        return artifact._call("trace.corpus.reachable", fields)
    fields["artifact"] = text_document(artifact, "artifact")
    return operation("trace.corpus.reachable", fields)


def walk_seed(
    seed: Path,
    *,
    cache: Optional[CachePolicy] = None,
    steps: int = 1_000_000,
    jobs: int = 1,
    limit: Optional[int] = None,
    cleanup: bool = False,
    self_check: bool = False,
    audit: bool = False,
    branches: Optional[Path] = None,
) -> Dict[str, Any]:
    """Explicit file read and optional branches output; core convergence checks."""
    return operation(
        "trace.corpus.walk_seed",
        {
            "seed": os.fsdecode(os.fspath(seed)),
            "cache": cache_request(cache),
            "steps": steps,
            "jobs": jobs,
            "limit": limit,
            "cleanup": cleanup,
            "self_check": self_check,
            "audit": audit,
            "branches": None if branches is None else os.fsdecode(os.fspath(branches)),
        },
    )


def fingerprint(
    artifact: JsonDoc,
    pc: int,
    state: int,
    *,
    descriptor: Optional[int] = None,
    steps: int = 1_000_000,
    parameters: Optional[Sequence[str]] = None,
    cleanup: bool = False,
) -> Dict[str, Any]:
    """Replay an explicit kernel thread (portable PrefixTraceReport projection)."""
    return operation(
        "trace.corpus.fingerprint",
        {
            "artifact": text_document(artifact, "artifact"),
            "pc": pc,
            "state": state,
            "descriptor": descriptor,
            "steps": steps,
            "parameters": None if parameters is None else list(parameters),
            "cleanup": cleanup,
        },
    )


def compare(baseline: JsonDoc, current: JsonDoc) -> Dict[str, Any]:
    return operation(
        "trace.corpus.compare",
        {
            "baseline": text_document(baseline, "baseline"),
            "current": text_document(current, "current"),
        },
    )


def compare_inline(outlined: JsonDoc, inline: JsonDoc) -> List[str]:
    return operation(
        "trace.corpus.compare_inline",
        {
            "outlined": text_document(outlined, "outlined"),
            "inline": text_document(inline, "inline"),
        },
    )

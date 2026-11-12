"""``shape.trace``: trace a whole program or one thread."""

from typing import Any, Dict, Optional, Union

from . import _trace_corpus as corpus
from ._native import JsonDoc, ShapeError, text_document
from ._native import operation as _call
from .cache import CachePolicy
from .cache import request as cache_request
from .kernel import LoadedKernel

__all__ = ["program", "thread"]

# {"fake": {"pc/state": bool}, "real": ["pc/state", ...]}
Overrides = Dict[str, Any]


def program(
    source: str,
    *,
    name: Optional[str] = None,
    steps: Optional[int] = None,
    thread_cleanup: bool = True,
    no_cleanup: bool = False,
    check_reuse: bool = False,
    overrides: Optional[Overrides] = None,
    jobs: Optional[int] = None,
    report_dir: Optional[str] = None,
    quiet: bool = True,
    cache: Optional[CachePolicy] = None,
) -> Dict[str, Any]:
    """Trace a raw capture into program JavaScript:
    ``{"code", "complete", "traced", "stopped": [{"function", "reason"}], "kernel_globals"}``."""
    if report_dir is not None:
        if cache is not None:
            raise ShapeError(
                "invalid_request", "use cache or legacy report_dir, not both"
            )
        cache = CachePolicy.directory(report_dir)
    return _call(
        "trace.program",
        {
            "source": source,
            "name": name,
            "steps": steps,
            "thread_cleanup": thread_cleanup,
            "no_cleanup": no_cleanup,
            "check_reuse": check_reuse,
            "overrides": overrides,
            "jobs": jobs,
            "cache": cache_request(cache),
            "quiet": quiet,
        },
    )


def thread(
    source: Optional[str] = None,
    *,
    artifact: Optional[Union[LoadedKernel, JsonDoc]] = None,
    pc: int,
    state: int,
    descriptor: Optional[int] = None,
    initial_slots: Optional[Any] = None,
    inputs: Optional[Any] = None,
    steps: Optional[int] = None,
    self_check: bool = False,
    outline: bool = True,
    reverse_sides: bool = False,
    partial: bool = False,
    overrides: Optional[Overrides] = None,
) -> Dict[str, Any]:
    """Walk one thread from ``pc``/``state``. Give exactly one of ``source``
    (a raw capture) or ``artifact`` (a kernel artifact, e.g.
    ``shape.deob(source, artifact=True)["artifact"]``)."""
    if (source is None) == (artifact is None):
        raise ShapeError(
            "invalid_request", "trace.thread needs exactly one of source or artifact"
        )
    request: Dict[str, Any] = {
        "source": source,
        "artifact": None
        if isinstance(artifact, LoadedKernel)
        else text_document(artifact, "artifact"),
        "pc": pc,
        "state": state,
        "descriptor": descriptor,
        "initial_slots": text_document(initial_slots, "initial_slots"),
        "inputs": text_document(inputs, "inputs"),
        "steps": steps,
        "self_check": self_check,
        "outline": outline,
        "reverse_sides": reverse_sides,
        "partial": partial,
        "overrides": overrides,
    }
    if isinstance(artifact, LoadedKernel):
        request.pop("artifact")
        return artifact._call("trace.thread", request)
    return _call("trace.thread", request)


def program_with_artifact(
    source: str,
    artifact: Union[LoadedKernel, JsonDoc],
    *,
    name: Optional[str] = None,
    steps: Optional[int] = None,
    thread_cleanup: bool = True,
    no_cleanup: bool = False,
    check_reuse: bool = False,
    overrides: Optional[Overrides] = None,
    jobs: Optional[int] = None,
    quiet: bool = True,
) -> Dict[str, Any]:
    """Trace matching raw source with an explicit kernel; no disk cache IO.

    A LoadedKernel reuses parsed native state; a document is parsed per call.
    """
    fields: Dict[str, Any] = {
        "source": source,
        "name": name,
        "steps": steps,
        "thread_cleanup": thread_cleanup,
        "no_cleanup": no_cleanup,
        "check_reuse": check_reuse,
        "overrides": overrides,
        "jobs": jobs,
        "quiet": quiet,
    }
    if isinstance(artifact, LoadedKernel):
        return artifact._call("trace.program", fields)
    fields["artifact"] = text_document(artifact, "artifact")
    return _call("trace.program", fields)


__all__ += ["program_with_artifact", "corpus"]

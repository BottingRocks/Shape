"""``shape.kernel``: the handler kernel of a capture."""

from typing import Any, Dict, Optional, Sequence

from ._generator import _Handle
from ._native import ArtifactInput, text_document
from ._native import operation as _call

__all__ = ["show"]


def show(
    source: str,
    operations: Optional[Sequence[str]] = None,
    input_kind: Optional[str] = None,
) -> Dict[str, Any]:
    """Kernel properties as tracer JSON. ``input_kind``: ``"raw"`` (default)
    or ``"deobfuscated"``; ``operations`` selects operations to include."""
    return _call(
        "kernel.show",
        {
            "source": source,
            "operations": None if operations is None else [*operations],
            "input_kind": input_kind,
        },
    )


class LoadedKernel(_Handle):
    """One parsed native kernel, reusable without disk cache lookups.

    Use a context manager or close() to release it after all active calls.
    """

    _family = "kernel"

    def to_json(self) -> str:
        return self._call("kernel.to_json", {})


def load(artifact: ArtifactInput) -> LoadedKernel:
    """Parse/validate a kernel document once; accepts JSON or PathLike."""
    return LoadedKernel.load(artifact)


def to_json(artifact: Any) -> str:
    """Serialize a loaded kernel or validate/serialize a kernel document."""
    if isinstance(artifact, LoadedKernel):
        return artifact.to_json()
    return _call("kernel.to_json", {"artifact": text_document(artifact, "artifact")})


__all__ += ["LoadedKernel", "load", "to_json"]

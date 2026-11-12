"""Python SDK of shape: deobfuscation, tracing and signal decoding.

A thin ``ctypes`` binding of the shape C ABI (``libshape``). Every call takes
plain Python values and returns parsed JSON (dicts); failures raise
:class:`ShapeError`. See ``README.md`` for library discovery and examples.

    import shape
    shape.version()                                  # '0.1.0'
    shape.cleanup.thread(js)["code"]                 # cleaned thread code
    with shape.Generator.load(artifact) as gen:
        decoded = gen.decode(headers)
"""

from typing import Any, Dict, Optional

from . import (
    artifact,
    cleanup,
    headers,
    kernel,
    naming,
    ops,
    payload,
    seeds,
    session,
    signals,
    trace,
)
from ._generator import Generator
from ._native import (
    JsonDoc,
    ShapeError,
    candidate_paths,
    library_path,
)
from ._native import (
    call as _call,
)
from ._native import (
    operation as _operation,
)
from ._native import (
    text_document as _doc,
)
from ._native import (
    version as _version,
)
from ._types import (
    CaptureHeaders,
    CompactSeedSchema,
    DecodedPayload,
    DecodeMetadata,
    NamedSignals,
    PackValue,
)
from .artifact import LoadedArtifact, decode, encode, encode_signals, explain, schema
from .cache import CachePolicy
from .superpack import pack, unpack, unpack_canonical

__all__ = [
    "ShapeError",
    "Generator",
    "version",
    "call",
    "commands",
    "deob",
    "report",
    "kernel",
    "ops",
    "trace",
    "cleanup",
    "naming",
    "signals",
    "pin",
    "session_from_initializer",
    "compile_artifact",
    "compact_schema",
    "library_path",
    "candidate_paths",
]

__all__ += [
    "artifact",
    "headers",
    "payload",
    "seeds",
    "session",
    "LoadedArtifact",
    "CachePolicy",
    "decode",
    "encode",
    "encode_signals",
    "explain",
    "schema",
    "pack",
    "unpack",
    "unpack_canonical",
    "version_info",
    "operation_commands",
    "CaptureHeaders",
    "CompactSeedSchema",
    "DecodedPayload",
    "DecodeMetadata",
    "NamedSignals",
    "PackValue",
]

__version__ = "0.1.0"


def version() -> str:
    """The native library's SDK version, e.g. ``"0.1.0"``."""
    return _version()


def call(command: str, **request: Any) -> Any:
    """Run any stateless command with snake_case request fields and return
    its ``value``: ``shape.call("deob", source=src)``. ``None`` fields are
    omitted, except ``artifact.compile`` requires a bool for ``direct``
    when provided."""
    if command == "artifact.compile" and "direct" in request:
        if not isinstance(request["direct"], bool):
            raise TypeError("direct must be a bool")
    return _call(command, request)


def commands() -> list:
    """The command names the loaded library accepts."""
    return _call("version", {})["commands"]


def deob(source: str, artifact: bool = False) -> Dict[str, Any]:
    """Deobfuscate a raw capture: ``{"code"}``; with ``artifact=True`` also
    the compiled kernel artifact under ``"artifact"``."""
    return _call("deob", {"source": source, "artifact": artifact})


def report(source: str, input_kind: Optional[str] = None) -> Dict[str, Any]:
    """The kernel artifact report. ``input_kind``: ``"raw"`` (default) or
    ``"deobfuscated"``."""
    return _call("report", {"source": source, "input_kind": input_kind})


def pin(
    source: str, session: Optional[JsonDoc] = None, init: Optional[str] = None
) -> Dict[str, Any]:
    """Pin the payload layout for a session. Give exactly one of ``session``
    (a session document) or ``init`` (initializer text)."""
    return _call(
        "pin", {"source": source, "session": _doc(session, "session"), "init": init}
    )


def session_from_initializer(init: str) -> Dict[str, Any]:
    """Build a session document from initializer text."""
    return _call("session.from_initializer", {"init": init})


def compile_artifact(
    source: str,
    session: JsonDoc,
    site_id: str,
    float_byte_order: Optional[str] = None,
    input_kind: Optional[str] = None,
    name: Optional[str] = None,
    direct: bool = False,
) -> Dict[str, Any]:
    """Compile a reusable decode artifact (load it with :meth:`Generator.load`).

    ``source`` is traced program JavaScript, or a raw capture with
    ``input_kind="raw"`` (traced first). ``float_byte_order``: ``"little"``
    or ``"big"``. ``direct=True`` opts into source-format decoding without
    lifecycle certification; the artifact retains this provenance."""
    if not isinstance(direct, bool):
        raise TypeError("direct must be a bool")
    return _call(
        "artifact.compile",
        {
            "source": source,
            "session": _doc(session, "session"),
            "site_id": site_id,
            "float_byte_order": float_byte_order,
            "input_kind": input_kind,
            "name": name,
            "direct": direct,
        },
    )


def compact_schema(artifact: JsonDoc) -> Dict[str, Any]:
    """The compact schema of a decode artifact (dict or JSON text)."""
    return _call("artifact.compact_schema", {"artifact": _doc(artifact, "artifact")})


def version_info() -> Dict[str, Any]:
    """v2 version, contract version, and complete portable operation names."""
    return _operation("version", {})


def operation_commands() -> list:
    """Complete named v2 operation IDs (legacy commands() remains compatible)."""
    return version_info()["commands"]

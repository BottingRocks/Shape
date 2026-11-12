"""Executable decode artifacts: compile/load once; export and persist explicitly."""

import ctypes
from typing import Any, Dict, Optional

from ._generator import Generator
from ._native import (
    ArtifactInput,
    JsonDoc,
    ShapeError,
    encode_json,
    encode_request,
    operation,
    symbol,
    text_document,
)
from ._types import CaptureHeaders, CompactSeedSchema, DecodedPayload
from .cache import CachePolicy
from .cache import request as cache_request


class LoadedArtifact(Generator):
    """One immutable, validated native artifact shared by direct operations.

    Inherited Generator methods remain available for compatibility. The native
    document is never exposed for mutation; exports are independent JSON bytes.
    """

    _family = "artifact"

    def to_json(self) -> bytes:
        """Export executable JSON explicitly; does not write a file."""
        return encode_json(self._call("artifact", {}))

    def artifact(self) -> Dict[str, Any]:
        """Return an independent document snapshot, never the live native state."""
        return self._call("artifact", {})


def load(document: ArtifactInput) -> LoadedArtifact:
    """Load JSON bytes/text, a parsed document, or an explicit PathLike file."""
    return LoadedArtifact.load(document)


def from_artifact(document: JsonDoc) -> LoadedArtifact:
    """Validate a document into one owned native handle."""
    return load(document)


def compile(
    source: str,
    session: JsonDoc,
    site_id: str,
    *,
    input_kind: str = "program",
    name: Optional[str] = None,
    cache: Optional[CachePolicy] = None,
    float_byte_order: str = "little",
    direct: bool = False,
) -> LoadedArtifact:
    """Compile traced program (or raw input) into a loaded executable artifact.

    The native compiler returns an already validated owner without a JSON
    export/reload cycle. Raw input uses full cleanup defaults. Persistence
    occurs only when callers explicitly export and save the artifact.
    """
    if not isinstance(direct, bool):
        raise TypeError("direct must be a bool")
    request = encode_request(
        {
            "source": source,
            "session": text_document(session, "session"),
            "site_id": site_id,
            "input_kind": input_kind,
            "name": name,
            "cache": cache_request(cache),
            "float_byte_order": float_byte_order,
            "direct": direct,
        }
    )
    error = ctypes.c_void_p()
    handle = symbol("shape_artifact_compile")(request, ctypes.byref(error))
    return LoadedArtifact._from_native(handle, error, "shape_artifact_compile")


def compact_schema(document: JsonDoc) -> CompactSeedSchema:
    """Describe executable JSON; the result is not a decoder."""
    return operation(
        "artifact.compact_schema", {"artifact": text_document(document, "artifact")}
    )


def to_json(loaded: LoadedArtifact) -> bytes:
    return loaded.to_json()


def _loaded(value: LoadedArtifact) -> LoadedArtifact:
    if not isinstance(value, LoadedArtifact):
        raise ShapeError(
            "invalid_request", "expected a LoadedArtifact; load executable JSON once"
        )
    return value


def decode(
    loaded: LoadedArtifact,
    capture: JsonDoc,
    session: Optional[JsonDoc] = None,
    *,
    verify: bool = False,
    metadata: bool = False,
) -> DecodedPayload:
    return _loaded(loaded).decode(capture, session, verify=verify, metadata=metadata)


def explain(
    loaded: LoadedArtifact,
    decoded: JsonDoc,
    signal: str,
    session: Optional[JsonDoc] = None,
    *,
    verify: bool = False,
) -> Dict[str, Any]:
    return _loaded(loaded).explain(decoded, signal, session, verify=verify)


def encode(
    loaded: LoadedArtifact, decoded: JsonDoc, session: Optional[JsonDoc] = None
) -> CaptureHeaders:
    return _loaded(loaded).encode(decoded=decoded, session=session)


def encode_signals(
    loaded: LoadedArtifact, signals: JsonDoc, session: Optional[JsonDoc] = None
) -> CaptureHeaders:
    return _loaded(loaded).encode(signals=signals, session=session)


def schema(loaded: LoadedArtifact) -> CompactSeedSchema:
    return _loaded(loaded).schema()

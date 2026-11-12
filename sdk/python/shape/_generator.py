"""``shape.Generator``: one loaded decode artifact, decoded many times."""

import ctypes
import os
import threading
from typing import Any, Dict, Optional, Type, TypeVar

from ._native import (
    ArtifactInput,
    JsonDoc,
    ShapeError,
    c_string,
    encode_json,
    encode_request,
    lib,
    parsed_document,
    symbol,
    take,
    text_document,
    unwrap,
)
from ._types import CaptureHeaders, CompactSeedSchema, DecodedPayload

__all__ = ["Generator"]

T = TypeVar("T", bound="_Handle")


class _Handle:
    """Common ownership and concurrency for immutable native artifact handles.

    Create it with :meth:`Generator.load`. One generator may be used from
    several threads at once; calls run in parallel (the GIL is released while
    the library works). Release it with :meth:`close` or a ``with`` block;
    it is also released when garbage-collected.
    """

    _family = "generator"

    def __init__(self, handle: int) -> None:
        # Use Generator.load(); this takes ownership of a native handle.
        self._handle: Optional[int] = handle
        self._active = 0
        self._closing = False
        self._cond = threading.Condition(threading.Lock())

    @classmethod
    def load(cls: Type[T], artifact: ArtifactInput) -> T:
        """Load a decode artifact: JSON ``bytes``/``str``, a parsed ``dict``,
        or an :class:`os.PathLike` naming a JSON file."""
        if isinstance(artifact, os.PathLike):
            with open(artifact, "rb") as f:
                data = f.read()
        elif isinstance(artifact, (bytes, bytearray)):
            data = bytes(artifact)
        elif isinstance(artifact, str):
            try:
                data = artifact.encode("utf-8")
            except UnicodeError as e:
                raise ShapeError("invalid_request", "artifact is not UTF-8 text") from e
        elif isinstance(artifact, dict):
            data = encode_json(artifact)
        else:
            raise ShapeError(
                "invalid_request",
                "artifact must be bytes, str, dict or a path, not {}".format(
                    type(artifact).__name__
                ),
            )
        native = lib()
        error = ctypes.c_void_p()
        loader = getattr(native, "shape_" + cls._family + "_load", None)
        if loader is None:
            loader = symbol("shape_" + cls._family + "_load")
        handle = loader(data, len(data), ctypes.byref(error))
        return cls._from_native(handle, error, "shape_" + cls._family + "_load")

    @classmethod
    def _from_native(
        cls: Type[T], handle: Optional[int], error: ctypes.c_void_p, operation: str
    ) -> T:
        """Adopt one produced native owner; free any owned error exactly once."""
        if not handle:
            if error.value:
                unwrap(take(error.value))  # raises ShapeError
            raise ShapeError("failed", "{} returned NULL".format(operation))
        if error.value:  # defensive: never leak an unexpected error string
            lib().shape_free(error.value)
        return cls(handle)

    # ---- lifetime ----------------------------------------------------------

    @property
    def closed(self) -> bool:
        """Whether new calls are rejected (including while close waits)."""
        with self._cond:
            return self._closing or self._handle is None

    def close(self) -> None:
        """Reject new calls, wait for active calls, and free the handle. Idempotent."""
        with self._cond:
            # Reject new calls before waiting, so a steady stream of calls
            # cannot starve close. Free under the lock so all closers wait.
            self._closing = True
            while self._active:
                self._cond.wait()
            handle, self._handle = self._handle, None
            if handle:
                getattr(lib(), "shape_" + self._family + "_free")(handle)

    def __enter__(self: T) -> T:
        if self.closed:
            raise ShapeError("invalid_request", "{} is closed".format(self._family))
        return self

    def __exit__(self, *exc: Any) -> None:
        self.close()

    def __del__(self) -> None:
        handle = getattr(self, "_handle", None)
        if handle:
            self._handle = None
            try:
                getattr(lib(), "shape_" + self._family + "_free")(handle)
            except Exception:  # interpreter shutdown
                pass

    def __repr__(self) -> str:
        return "<shape.{} {}>".format(
            type(self).__name__, "closed" if self.closed else "open"
        )

    # ---- calls -------------------------------------------------------------

    def _call(self, method: str, request: Dict[str, Any]) -> Any:
        method_bytes = c_string(method, "method")
        body = encode_request(request)
        with self._cond:
            handle = self._handle
            if self._closing or handle is None:
                raise ShapeError("invalid_request", "{} is closed".format(self._family))
            self._active += 1
        try:
            if self._family == "artifact" and method in (
                "decode",
                "encode",
                "explain",
                "schema",
            ):
                text = take(symbol("shape_" + method)(handle, body))
            else:
                text = take(
                    getattr(lib(), "shape_" + self._family + "_call")(
                        handle, method_bytes, body
                    )
                )
        finally:
            with self._cond:
                self._active -= 1
                if not self._active:
                    self._cond.notify_all()
        return unwrap(text)

    def call(self, method: str, **request: Any) -> Any:
        """Generic escape hatch: ``decode``, ``explain``, ``encode``, ``schema``."""
        return self._call(method, request)


class Generator(_Handle):
    """Compatibility interface: loaded decode artifact with method operations."""

    def decode(
        self,
        capture: JsonDoc,
        session: Optional[JsonDoc] = None,
        verify: bool = False,
        metadata: bool = False,
    ) -> DecodedPayload:
        """Captured headers (``{"siteId", "headers": {...}}``) -> named signals."""
        return self._call(
            "decode",
            {
                "capture": text_document(capture, "capture"),
                "session": text_document(session, "session"),
                "verify": verify,
                "metadata": metadata,
            },
        )

    def explain(
        self,
        decoded: JsonDoc,
        signal: str,
        session: Optional[JsonDoc] = None,
        verify: bool = False,
    ) -> Dict[str, Any]:
        """One signal's full metadata, from a :meth:`decode` result."""
        return self._call(
            "explain",
            {
                "decoded": text_document(decoded, "decoded"),
                "signal": signal,
                "session": text_document(session, "session"),
                "verify": verify,
            },
        )

    def encode(
        self,
        signals: Optional[JsonDoc] = None,
        decoded: Optional[JsonDoc] = None,
        session: Optional[JsonDoc] = None,
    ) -> CaptureHeaders:
        """Named signals -> headers. Pass exactly one of ``signals``
        (``positionalSignals``) or ``decoded`` (a full :meth:`decode` result)."""
        return self._call(
            "encode",
            {
                # Accept a document or JSON text; parse text here (exactly) so
                # the request carries an object.
                "signals": None
                if signals is None
                else parsed_document(signals, "signals"),
                "decoded": None
                if decoded is None
                else parsed_document(decoded, "decoded"),
                "session": text_document(session, "session"),
            },
        )

    def schema(self) -> CompactSeedSchema:
        """The artifact's compact schema."""
        return self._call("schema", {})

"""ctypes binding of the shape C ABI (``sdk/c/include/shape.h``).

The library is loaded lazily on first use, so ``import shape`` never fails
because the native library is missing; the first call raises a clear error.
The library is opened with :class:`ctypes.CDLL`, which releases the GIL for
the duration of every foreign call: several Python threads can run shape
calls in parallel.

JSON integers are exact. Decode artifacts and decoded payloads carry 64-bit
values (f64 bit patterns, u64 superpack values) above 2^53. ``json.loads`` parses
integer tokens as arbitrary-precision :class:`int`, and this module never
converts them to ``float``; :func:`encode_request` serializes those ints back
as the same decimal digits. Documents therefore round-trip the JSON boundary
unchanged whether they are passed as parsed values or as JSON text.
"""

import ctypes
import json
import os
import sys
import threading
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

from ._types import CaptureHeaders, CompactSeedSchema, DecodedPayload, PackValue

__all__ = ["ShapeError", "library_path", "candidate_paths", "call", "lib"]

JsonDoc = Union[
    Dict[str, Any],
    List[Any],
    str,
    bytes,
    bytearray,
    CaptureHeaders,
    DecodedPayload,
    CompactSeedSchema,
    PackValue,
]


ArtifactInput = Union[JsonDoc, os.PathLike[str]]


class ShapeError(Exception):
    """A failed shape call.

    ``code`` is one of ``invalid_request``, ``unknown_command``, ``failed``,
    ``panic`` (from the library) or ``library_not_found`` (raised by this
    binding when the native library cannot be located) or
    ``unsupported_library`` when a named v2 symbol is missing.
    """

    def __init__(self, code: str, message: str) -> None:
        super().__init__(code, message)
        self.code = code
        self.message = message

    def __str__(self) -> str:
        return "{}: {}".format(self.code, self.message)


def _library_names() -> List[str]:
    if sys.platform == "darwin":
        return ["libshape.dylib"]
    if sys.platform.startswith("win") or sys.platform == "cygwin":
        return ["shape.dll"]
    return ["libshape.so"]


def _repo_root() -> Optional[Path]:
    """The first ancestor of the package directory that contains ``sdk/``."""
    here = Path(__file__).resolve().parent
    for parent in here.parents:
        if (parent / "sdk").is_dir():
            return parent
    return None


def candidate_paths() -> List[Path]:
    """Where the library is looked for, in order (``SHAPE_LIBRARY`` excluded)."""
    folders = [Path(__file__).resolve().parent / "lib"]
    root = _repo_root()
    if root is not None:
        folders += [root / "target" / "release", root / "target" / "iter"]
    return [folder / name for folder in folders for name in _library_names()]


def library_path() -> Path:
    """Resolve the native library path, or raise :class:`ShapeError`."""
    override = os.environ.get("SHAPE_LIBRARY")
    if override:
        path = Path(override)
        if not path.is_file():
            raise ShapeError(
                "library_not_found",
                "SHAPE_LIBRARY={} does not name a file".format(override),
            )
        return path.resolve()
    candidates = candidate_paths()
    for path in candidates:
        if path.is_file():
            return path
    raise ShapeError(
        "library_not_found",
        "shape native library not found. Set SHAPE_LIBRARY to the full path of {}, "
        "or place it in one of: {}".format(
            " / ".join(_library_names()), ", ".join(str(p) for p in candidates)
        ),
    )


_lib: Optional[ctypes.CDLL] = None
_lib_lock = threading.Lock()


def lib() -> ctypes.CDLL:
    """The loaded library, with argument and result types declared."""
    global _lib
    if _lib is not None:
        return _lib
    with _lib_lock:
        if _lib is None:
            path = library_path()
            try:
                handle = ctypes.CDLL(str(path))
            except OSError as e:
                raise ShapeError(
                    "library_not_found", "cannot load {}: {}".format(path, e)
                ) from e
            c_char_p, c_void_p = ctypes.c_char_p, ctypes.c_void_p
            handle.shape_version.argtypes = []
            handle.shape_version.restype = c_char_p  # static: never freed
            # Owned results are c_void_p so the pointer survives for shape_free.
            handle.shape_call.argtypes = [c_char_p, c_char_p]
            handle.shape_call.restype = c_void_p
            handle.shape_generator_load.argtypes = [
                c_char_p,
                ctypes.c_size_t,
                ctypes.POINTER(c_void_p),
            ]
            handle.shape_generator_load.restype = c_void_p
            handle.shape_generator_call.argtypes = [c_void_p, c_char_p, c_char_p]
            handle.shape_generator_call.restype = c_void_p
            handle.shape_generator_free.argtypes = [c_void_p]
            handle.shape_generator_free.restype = None
            handle.shape_free.argtypes = [c_void_p]
            handle.shape_free.restype = None
            # v2 symbols are resolved on use so legacy Generator/call can still
            # work against an older native library. Named operations never fall
            # back silently to legacy defaults.
            for name in ("shape_call_v2",):
                if hasattr(handle, name):
                    getattr(handle, name).argtypes = [c_char_p, c_char_p]
                    getattr(handle, name).restype = c_void_p
            if hasattr(handle, "shape_artifact_compile"):
                handle.shape_artifact_compile.argtypes = [
                    c_char_p,
                    ctypes.POINTER(c_void_p),
                ]
                handle.shape_artifact_compile.restype = c_void_p
            for family in ("artifact", "kernel"):
                for suffix, args, result in (
                    (
                        "load",
                        [c_char_p, ctypes.c_size_t, ctypes.POINTER(c_void_p)],
                        c_void_p,
                    ),
                    ("call", [c_void_p, c_char_p, c_char_p], c_void_p),
                    ("free", [c_void_p], None),
                ):
                    name = "shape_{}_{}".format(family, suffix)
                    if hasattr(handle, name):
                        getattr(handle, name).argtypes = args
                        getattr(handle, name).restype = result
            for name in ("decode", "encode", "explain", "schema"):
                symbol = "shape_" + name
                if hasattr(handle, symbol):
                    getattr(handle, symbol).argtypes = [c_void_p, c_char_p]
                    getattr(handle, symbol).restype = c_void_p
            _lib = handle
    return _lib


def take(pointer: Optional[int]) -> bytes:
    """Copy an owned library string and release it, whatever happens."""
    if not pointer:
        raise ShapeError("failed", "shape returned NULL")
    try:
        return ctypes.string_at(pointer)
    finally:
        lib().shape_free(pointer)


def unwrap(text: bytes) -> Any:
    """Envelope bytes -> ``value``, or raise :class:`ShapeError`.

    Integer tokens become Python :class:`int` (never ``float``), so 64-bit
    values in artifacts and payloads keep their exact value.
    """
    try:
        envelope = json.loads(text)
    except ValueError as e:
        raise ShapeError("failed", "shape returned invalid JSON: {}".format(e)) from e
    if not isinstance(envelope, dict) or not isinstance(envelope.get("ok"), bool):
        raise ShapeError("failed", "shape returned a malformed envelope")
    if envelope["ok"]:
        if "value" not in envelope:
            raise ShapeError("failed", "success envelope without value")
        return envelope["value"]
    error = envelope.get("error")
    if not isinstance(error, dict):
        raise ShapeError("failed", "error envelope without error")
    raise ShapeError(str(error.get("code", "failed")), str(error.get("message", "")))


def encode_request(request: Dict[str, Any]) -> bytes:
    """Request dict -> JSON bytes; ``None`` values are dropped (absent).

    ``int`` values (including ones above 2^53) are written as exact decimal
    digits: no float conversion happens on the request path.
    """
    body = {k: v for k, v in request.items() if v is not None}
    return encode_json(body)


def encode_json(value: Any) -> bytes:
    """Serialize strict JSON while retaining exact integer literals."""
    try:
        return json.dumps(value, ensure_ascii=False, allow_nan=False).encode("utf-8")
    except (TypeError, ValueError, UnicodeError) as e:
        raise ShapeError(
            "invalid_request", "request is not JSON-serializable: {}".format(e)
        ) from e


def c_string(value: str, name: str) -> bytes:
    """Validate text sent through a NUL-terminated C ABI argument."""
    if not isinstance(value, str) or "\0" in value:
        raise ShapeError("invalid_request", "{} must be text without NUL".format(name))
    try:
        return value.encode("utf-8")
    except UnicodeError as e:
        raise ShapeError("invalid_request", "{} is not UTF-8 text".format(name)) from e


def call(command: str, request: Dict[str, Any]) -> Any:
    """Run a stateless command and return its ``value``."""
    command_bytes = c_string(command, "command")
    body = encode_request(request)
    native = lib()
    return unwrap(take(native.shape_call(command_bytes, body)))


def version() -> str:
    return lib().shape_version().decode("utf-8")


def text_document(value: Any, name: str) -> Any:
    """A document argument as the library accepts it: dicts/lists as-is,
    JSON text (``str`` or ``bytes``) passed through as a string that the
    library parses itself."""
    if isinstance(value, (bytes, bytearray)):
        try:
            return bytes(value).decode("utf-8")
        except UnicodeError as e:
            raise ShapeError(
                "invalid_request", "{!r} is not UTF-8 text".format(name)
            ) from e
    if isinstance(value, os.PathLike):
        raise ShapeError(
            "invalid_request",
            "{!r} must be a dict or JSON text, not a path; read the file first".format(
                name
            ),
        )
    return value


def parsed_document(value: Any, name: str) -> Any:
    """A document argument parsed in Python (for fields the library does not
    parse from a string itself)."""
    value = text_document(value, name)
    if isinstance(value, str):
        try:
            return json.loads(value)
        except ValueError as e:
            raise ShapeError(
                "invalid_request", "{!r} is not JSON: {}".format(name, e)
            ) from e
    return value


def symbol(name: str) -> Any:
    """Resolve v2 explicitly; a stale library must never change semantics."""
    try:
        return getattr(lib(), name)
    except AttributeError as e:
        raise ShapeError(
            "unsupported_library",
            "native library lacks {}; rebuild libshape for contract v2".format(name),
        ) from e


def operation(command: str, request: Dict[str, Any]) -> Any:
    """Run a named operation using authoritative contract v2."""
    command_bytes = c_string(command, "operation")
    body = encode_request(request)
    return unwrap(take(symbol("shape_call_v2")(command_bytes, body)))


def byte_array(
    value: Union[bytes, bytearray, memoryview], name: str = "bytes"
) -> List[int]:
    """Convert explicit opaque bytes to the contract's u8 array."""
    if not isinstance(value, (bytes, bytearray, memoryview)):
        raise ShapeError(
            "invalid_request", "{} must be bytes, bytearray or memoryview".format(name)
        )
    return list(bytes(value))

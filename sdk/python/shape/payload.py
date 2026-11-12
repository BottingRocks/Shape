"""Opaque payload framing and generic Report channels with explicit profiles."""

from typing import Any, Dict, Optional, Sequence, Union

from ._native import JsonDoc, byte_array, operation
from ._native import text_document as doc


def codec(profile: JsonDoc) -> Dict[str, Any]:
    return operation("payload.codec", {"profile": doc(profile, "profile")})


def encode(profile: JsonDoc, report: JsonDoc) -> str:
    return operation(
        "payload.encode",
        {"profile": doc(profile, "profile"), "report": doc(report, "report")},
    )


def decode(
    profile: JsonDoc,
    text: str,
    positional_count: int,
    keysets: Optional[Sequence[Sequence[str]]] = None,
) -> Dict[str, Any]:
    return operation(
        "payload.decode",
        {
            "profile": doc(profile, "profile"),
            "text": text,
            "positional_count": positional_count,
            "keysets": keysets,
        },
    )


def encode_bytes(
    profile: JsonDoc, data: Union[bytes, bytearray, memoryview], seeds: Sequence[int]
) -> str:
    return operation(
        "payload.encode_bytes",
        {
            "profile": doc(profile, "profile"),
            "bytes": byte_array(data),
            "seeds": list(seeds),
        },
    )


def decode_bytes(profile: JsonDoc, text: str) -> Dict[str, Any]:
    """Return explicit Payload (seeds, data, headerContext), not named signals."""
    return operation(
        "payload.decode_bytes", {"profile": doc(profile, "profile"), "text": text}
    )

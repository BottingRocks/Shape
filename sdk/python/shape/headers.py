"""Full-name header framing with explicit profiles (not suffix CaptureHeaders)."""

from typing import Any, Dict, Optional, Sequence

from ._native import JsonDoc, operation
from ._native import text_document as doc


def codec(profile: JsonDoc) -> Dict[str, Any]:
    """Validate and return a profile for explicit use in subsequent calls."""
    return operation("headers.codec", {"profile": doc(profile, "profile")})


def parse(text: str) -> Dict[str, str]:
    return operation("headers.parse", {"text": text})


def encode(profile: JsonDoc, report: JsonDoc) -> Dict[str, str]:
    return operation(
        "headers.encode",
        {"profile": doc(profile, "profile"), "report": doc(report, "report")},
    )


def decode(
    profile: JsonDoc,
    headers: JsonDoc,
    positional_count: int,
    keysets: Optional[Sequence[Sequence[str]]] = None,
) -> Dict[str, Any]:
    return operation(
        "headers.decode",
        {
            "profile": doc(profile, "profile"),
            "headers": doc(headers, "headers"),
            "positional_count": positional_count,
            "keysets": keysets,
        },
    )


def encode_bytes(profile: JsonDoc, payload: JsonDoc) -> Dict[str, str]:
    return operation(
        "headers.encode_bytes",
        {"profile": doc(profile, "profile"), "payload": doc(payload, "payload")},
    )


def decode_bytes(profile: JsonDoc, headers: JsonDoc) -> Dict[str, Any]:
    return operation(
        "headers.decode_bytes",
        {"profile": doc(profile, "profile"), "headers": doc(headers, "headers")},
    )


def encode_finalized(
    profile: JsonDoc, text: str, context: Optional[JsonDoc] = None
) -> Dict[str, str]:
    return operation(
        "headers.encode_finalized",
        {
            "profile": doc(profile, "profile"),
            "text": text,
            "context": doc(context, "context"),
        },
    )


def decode_finalized(profile: JsonDoc, headers: JsonDoc) -> Dict[str, Any]:
    return operation(
        "headers.decode_finalized",
        {"profile": doc(profile, "profile"), "headers": doc(headers, "headers")},
    )

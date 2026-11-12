"""Public domain document types; field names follow published engine JSON."""

from typing import Any, Dict, List, TypedDict


class CaptureHeaders(TypedDict):
    """Site identity and suffix-keyed headers (not full-name framed headers)."""

    siteId: str
    headers: Dict[str, str]


class _PackKind(TypedDict):
    kind: str


class PackValue(_PackKind, total=False):
    """Tagged superpack value; number value is 16 hexadecimal IEEE bits."""

    value: Any


NamedSignals = Dict[str, Dict[str, Any]]


class DecodeMetadata(TypedDict):
    originalReport: Dict[str, Any]
    signals: Dict[str, Any]
    decoded: int
    missing: int
    exactReencode: bool
    floatByteOrder: str


class DecodedPayload(TypedDict):
    """Editable named fields plus retained original channel/encoding context."""

    siteId: str
    errors: List[PackValue]
    positionalSignals: NamedSignals
    signals: List[PackValue]
    keyedSignals: List[Any]
    seeds: List[int]
    keysets: List[List[str]]
    metadata: DecodeMetadata


class CompactSeedSchema(TypedDict):
    """Descriptive export only; cannot be loaded as a decoder."""

    format: str
    seedId: str
    encoding: Dict[str, Any]
    keysets: List[List[str]]
    signals: Dict[str, Any]
    reportRoutes: Dict[str, Any]
    unroutedSignals: List[str]

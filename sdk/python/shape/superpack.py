"""Typed superpack values preserve undefined/binary/dates and IEEE number bits."""

from typing import Any, Dict, Optional, Union

from ._native import JsonDoc, byte_array, operation
from ._native import text_document as doc
from ._types import PackValue


def pack(value: JsonDoc, options: Optional[JsonDoc] = None) -> bytes:
    return bytes(
        operation(
            "pack", {"value": doc(value, "value"), "options": doc(options, "options")}
        )
    )


def unpack(
    data: Union[bytes, bytearray, memoryview], options: Optional[JsonDoc] = None
) -> Dict[str, Any]:
    return operation(
        "unpack", {"bytes": byte_array(data), "options": doc(options, "options")}
    )


def unpack_canonical(
    data: Union[bytes, bytearray, memoryview], options: Optional[JsonDoc] = None
) -> PackValue:
    return operation(
        "unpack_canonical",
        {"bytes": byte_array(data), "options": doc(options, "options")},
    )

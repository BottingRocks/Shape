"""Explicit source classification, URL identity, acquisition and persistence.

Only fetch/fetch_direct use the network; only import_seed writes a store.
Portable clients are bounded configuration documents, not HTTP trait objects.
"""

import os
from typing import Any, Dict, List, Optional, Sequence, Union

from ._native import byte_array, operation

Bytes = Union[bytes, bytearray, memoryview]


def parse(data: Bytes) -> Dict[str, Any]:
    return operation("seeds.parse", {"bytes": byte_array(data)})


def parse_launcher(data: Bytes) -> Dict[str, Any]:
    return operation("seeds.parse_launcher", {"bytes": byte_array(data)})


def parse_kernel(data: Bytes) -> Dict[str, Any]:
    return operation("seeds.parse_kernel", {"bytes": byte_array(data)})


def parse_url(input: str) -> str:
    return operation("seeds.parse_url", {"input": input})


def resolve_url(reference: str, base: Optional[str] = None) -> str:
    return operation("seeds.resolve_url", {"reference": reference, "base": base})


def token(reference: str, param: str = "seed") -> str:
    return operation("seeds.token", {"reference": reference, "param": param})


def sites() -> List[Dict[str, Any]]:
    return operation("seeds.sites", {})


def site(name: str) -> Dict[str, Any]:
    return operation("seeds.site", {"name": name})


def client(
    *,
    timeout_ms: Optional[int] = None,
    connect_timeout_ms: Optional[int] = None,
    max_body_bytes: Optional[int] = None,
    max_redirects: Optional[int] = None,
) -> Dict[str, Any]:
    """Validate/normalize configuration; does not perform a network request."""
    return operation(
        "seeds.client",
        {
            "timeout_ms": timeout_ms,
            "connect_timeout_ms": connect_timeout_ms,
            "max_body_bytes": max_body_bytes,
            "max_redirects": max_redirects,
        },
    )


def fetch(
    url: str,
    *,
    client: Optional[Dict[str, Any]] = None,
    headers: Optional[Sequence[Sequence[str]]] = None,
    base_url: Optional[str] = None,
    seed_param: Optional[str] = None,
) -> Dict[str, Any]:
    """Explicit bounded launcher/kernel acquisition; never saves a store."""
    return operation(
        "seeds.fetch",
        {
            "url": url,
            "client": client,
            "headers": headers,
            "base_url": base_url,
            "seed_param": seed_param,
        },
    )


def fetch_direct(
    url: str,
    *,
    client: Optional[Dict[str, Any]] = None,
    headers: Optional[Sequence[Sequence[str]]] = None,
    base_url: Optional[str] = None,
    seed_param: Optional[str] = None,
) -> Dict[str, Any]:
    """Explicit bounded direct-kernel acquisition; never saves a store."""
    return operation(
        "seeds.fetch_direct",
        {
            "url": url,
            "client": client,
            "headers": headers,
            "base_url": base_url,
            "seed_param": seed_param,
        },
    )


def import_seed(
    directory: Union[str, os.PathLike],
    seed: str,
    kernel: Bytes,
    *,
    launcher: Optional[Bytes] = None,
) -> Dict[str, Any]:
    """Validate bytes and persist to an explicit destination.

    Caller supplies the source-to-token association. Python's import keyword
    requires this idiomatic spelling of Rust seeds::import.
    """
    return operation(
        "seeds.import",
        {
            "directory": os.fsdecode(os.fspath(directory)),
            "seed": seed,
            "kernel": byte_array(kernel, "kernel"),
            "launcher": None if launcher is None else byte_array(launcher, "launcher"),
        },
    )

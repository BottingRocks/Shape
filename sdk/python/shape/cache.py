"""Explicit per-call kernel cache policy; never changes process environment."""

import os
from dataclasses import dataclass
from typing import Dict, Optional, Union

from ._native import ShapeError

Path = Union[str, os.PathLike]


@dataclass(frozen=True)
class CachePolicy:
    """Disabled by default; directory and environment are explicit opt-ins."""

    mode: str = "disabled"
    directory_path: Optional[str] = None

    def __post_init__(self) -> None:
        if self.mode not in ("disabled", "directory", "environment"):
            raise ShapeError("invalid_request", "unknown cache policy")
        if self.mode == "directory":
            if (
                not isinstance(self.directory_path, str)
                or not self.directory_path.strip()
            ):
                raise ShapeError(
                    "invalid_request", "directory cache requires a nonempty path"
                )
        elif self.directory_path is not None:
            raise ShapeError("invalid_request", "only directory cache accepts a path")

    @classmethod
    def disabled(cls) -> "CachePolicy":
        return cls()

    @classmethod
    def directory(cls, path: Path) -> "CachePolicy":
        return cls("directory", os.fsdecode(os.fspath(path)))

    @classmethod
    def environment(cls) -> "CachePolicy":
        return cls("environment")

    def to_request(self) -> Dict[str, str]:
        result = {"mode": self.mode}
        if self.directory_path is not None:
            result["directory"] = self.directory_path
        return result


def request(policy: Optional[CachePolicy]) -> Dict[str, str]:
    if policy is None:
        return {"mode": "disabled"}
    if not isinstance(policy, CachePolicy):
        raise ShapeError("invalid_request", "cache must be a CachePolicy")
    return policy.to_request()

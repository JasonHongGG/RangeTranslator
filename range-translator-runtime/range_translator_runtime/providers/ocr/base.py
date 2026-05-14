from __future__ import annotations

from typing import Any, Protocol


class OcrProvider(Protocol):
    id: str
    label: str

    def descriptor(self) -> dict[str, Any]: ...

    def recognize(self, payload: dict[str, Any]) -> dict[str, Any]: ...

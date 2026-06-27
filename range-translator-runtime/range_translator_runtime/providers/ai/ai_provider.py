from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

@dataclass
class GenerateRequest:
    prompt: str
    system_prompt: str | None = None
    temperature: float | None = None
    max_tokens: int | None = None
    images: list[str] | None = None
    session_id: str | None = None
    stream: bool | None = False

@dataclass
class GenerateResponse:
    text: str
    usage: dict[str, int] | None = None
    metadata: dict[str, Any] = field(default_factory=dict)

class AIProvider:
    @property
    def name(self) -> str:
        raise NotImplementedError

    def generate(self, request: GenerateRequest) -> GenerateResponse:
        raise NotImplementedError

    def descriptor(self) -> dict[str, Any]:
        return {
            "id": self.name,
            "label": self.name.capitalize(),
            "kind": "ai",
            "available": True,
            "detail": None,
        }

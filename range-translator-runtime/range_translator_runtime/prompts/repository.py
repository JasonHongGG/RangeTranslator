from __future__ import annotations

import json
from pathlib import Path

from range_translator_runtime.runtime_types import PromptPayload


class PromptRepository:
    def __init__(self, root: Path) -> None:
        self.root = root
        self._cache: dict[str, PromptPayload] | None = None

    def list_profiles(self) -> list[dict[str, str]]:
        return [
            {
                "id": prompt["id"],
                "label": prompt["label"],
                "version": prompt["version"],
                "task": prompt["task"],
                "providerFamily": prompt["providerFamily"],
            }
            for prompt in self._load_all().values()
        ]

    def load(self, prompt_id: str) -> PromptPayload:
        try:
            return self._load_all()[prompt_id]
        except KeyError as error:
            raise RuntimeError(f"Prompt profile not found: {prompt_id}") from error

    def _load_all(self) -> dict[str, PromptPayload]:
        if self._cache is not None:
            return self._cache

        prompts: dict[str, PromptPayload] = {}
        for candidate in sorted(self.root.rglob("*.json")):
            payload = json.loads(candidate.read_text(encoding="utf-8"))
            prompts[payload["id"]] = payload

        self._cache = prompts
        return prompts

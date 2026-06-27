from __future__ import annotations

import logging
from typing import Any

from range_translator_runtime.providers.ai import AIProvider

class BaseAgent:
    def __init__(self, name: str, provider: AIProvider) -> None:
        self.name = name
        self.provider = provider
        self.logger = logging.getLogger(f"agent.{name}")

    def execute(self, *args: Any, **kwargs: Any) -> Any:
        raise NotImplementedError

from __future__ import annotations

from typing import Any
from range_translator_runtime.core import EventEmitter, JsonMap
from .context import AppContext

class Dispatcher:
    def __init__(self, context: AppContext) -> None:
        self.context = context

    def dispatch(
        self,
        subcommand: str,
        payload: JsonMap,
        emit_event: EventEmitter | None = None,
    ) -> JsonMap:
        if subcommand == "status":
            return self.context.get_status()
            
        if subcommand == "prewarm":
            provider = self.context.get_ocr_provider(payload.get("providerId"))
            return provider.prewarm(payload)
            
        if subcommand == "recognize":
            provider = self.context.get_ocr_provider(payload.get("providerId"))
            return provider.recognize(payload)
            
        if subcommand == "translate":
            return self.context.translator_agent.execute(payload, emit_event)
            
        raise RuntimeError(f"Unsupported subcommand: {subcommand}")

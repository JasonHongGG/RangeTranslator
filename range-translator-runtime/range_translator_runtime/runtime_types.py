from __future__ import annotations

from typing import Any, Callable

PromptPayload = dict[str, Any]
EventEmitter = Callable[[str, dict[str, Any]], None]

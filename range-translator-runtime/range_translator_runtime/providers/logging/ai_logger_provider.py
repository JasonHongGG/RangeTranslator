from __future__ import annotations

import json
import secrets
from datetime import datetime, timezone
from pathlib import Path
from time import perf_counter
from typing import Any

from range_translator_runtime.core.config import Config

def _utc_now() -> datetime:
    return datetime.now(timezone.utc)

def _timestamp_for_filename() -> str:
    return _utc_now().strftime("%Y%m%d_%H%M%S")

def _timestamp_for_json() -> str:
    return _utc_now().isoformat().replace("+00:00", "Z")

def _runtime_root() -> Path:
    return Path(__file__).resolve().parents[3]

def _ai_log_dir() -> Path:
    override = Config.get_ai_log_dir()
    root = Path(override) if override else _runtime_root() / ".runtime" / "ai-log"
    root.mkdir(parents=True, exist_ok=True)
    return root

def _log_path(kind: str) -> Path:
    return _ai_log_dir() / f"{_timestamp_for_filename()}_{kind}_{secrets.token_hex(4)}.json"

class AILoggerProvider:
    """
    A generic provider for logging AI interactions (Prompts, Model IO, Latency).
    """
    def __init__(self, action_name: str, payload: dict[str, Any], prompt_metadata: dict[str, Any]) -> None:
        self.path = _log_path(action_name)
        self._started_perf = perf_counter()
        self._finalized = False
        
        self.document: dict[str, Any] = {
            "metadata": {
                "timestamp": _timestamp_for_json(),
                "action": action_name,
                "requestId": payload.get("_rtRequestId"),
                "status": "started",
                "latencyMs": None,
                "custom": {},
            },
            "request": {
                "payload": self._sanitize(payload),
                "promptMetadata": prompt_metadata,
                "chatRequests": [],
            },
            "response": {
                "partialEvents": [],
                "modelOutputs": [],
                "finalResult": None,
                "error": None,
            },
        }

    def _sanitize(self, payload: dict[str, Any]) -> dict[str, Any]:
        return {k: v for k, v in payload.items() if not str(k).startswith("_rt")}

    def set_custom_metadata(self, key: str, value: Any) -> None:
        self.document["metadata"]["custom"][key] = value

    def add_chat_request(self, attempt: int, payload: dict[str, Any]) -> None:
        self.document["request"]["chatRequests"].append({
            "attempt": attempt,
            "payload": payload,
        })

    def add_model_output(self, attempt: int, content: str) -> None:
        self.document["response"]["modelOutputs"].append({
            "attempt": attempt,
            "content": content,
        })

    def add_partial_event(self, payload: dict[str, Any]) -> None:
        self.document["response"]["partialEvents"].append(payload)

    def finalize_success(self, result: dict[str, Any]) -> None:
        self.document["metadata"]["latencyMs"] = round((perf_counter() - self._started_perf) * 1000.0, 3)
        self.document["metadata"]["status"] = "success"
        self.document["response"]["finalResult"] = result
        self._write()

    def finalize_error(self, error_message: str, traceback_text: str) -> None:
        self.document["metadata"]["latencyMs"] = round((perf_counter() - self._started_perf) * 1000.0, 3)
        self.document["metadata"]["status"] = "error"
        self.document["response"]["error"] = {
            "message": error_message,
            "traceback": traceback_text,
        }
        self._write()

    def _write(self) -> None:
        if self._finalized:
            return
        self._finalized = True
        try:
            temp_path = self.path.with_suffix(self.path.suffix + ".tmp")
            temp_path.write_text(json.dumps(self.document, ensure_ascii=False, indent=2), encoding="utf-8")
            temp_path.replace(self.path)
        except OSError:
            pass

from __future__ import annotations

import json
import os
import secrets
from datetime import datetime, timezone
from pathlib import Path
from time import perf_counter
from typing import Any


def _utc_now() -> datetime:
    return datetime.now(timezone.utc)


def _timestamp_for_filename() -> str:
    return _utc_now().strftime("%Y%m%d_%H%M%S")


def _timestamp_for_json() -> str:
    return _utc_now().isoformat().replace("+00:00", "Z")


def _runtime_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _ai_log_dir() -> Path:
    override = os.environ.get("RANGE_TRANSLATOR_AI_LOG_DIR")
    root = Path(override) if override else _runtime_root() / ".runtime" / "ai-log"
    root.mkdir(parents=True, exist_ok=True)
    return root


def _log_path(kind: str) -> Path:
    return _ai_log_dir() / f"{_timestamp_for_filename()}_{kind}_{secrets.token_hex(4)}.json"


def _sanitize_request_payload(payload: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in payload.items()
        if not str(key).startswith("_rt")
    }


class TranslateAiLog:
    def __init__(self, payload: dict[str, Any], prompt: dict[str, Any]) -> None:
        sanitized_payload = _sanitize_request_payload(payload)
        self.path = _log_path("translate")
        self._started_perf = perf_counter()
        self._finalized = False
        self.document: dict[str, Any] = {
            "metadata": {
                "timestamp": _timestamp_for_json(),
                "requestId": payload.get("_rtRequestId"),
                "providerId": sanitized_payload.get("providerId") or "ollama",
                "model": sanitized_payload.get("model") or "discovering",
                "promptProfile": prompt.get("id"),
                "sourceLanguage": sanitized_payload.get("sourceLanguage") or "auto",
                "targetLanguage": sanitized_payload.get("targetLanguage") or "zh-TW",
                "itemCount": len(list(sanitized_payload.get("items") or [])),
                "expectedItemCount": int(
                    sanitized_payload.get("expectedItemCount")
                    or len(list(sanitized_payload.get("items") or []))
                ),
                "repairCount": 0,
                "latencyMs": None,
                "status": "started",
            },
            "request": {
                "normalizedPayload": sanitized_payload,
                "prompt": {
                    "id": prompt.get("id"),
                    "system": prompt.get("system") or prompt.get("systemPrompt"),
                    "translationTemplate": prompt.get("translationTemplate") or prompt.get("userTemplate"),
                    "repairTemplate": prompt.get("repairTemplate"),
                    "outputSchema": prompt.get("outputSchema"),
                    "styleDirectives": prompt.get("styleDirectives"),
                    "qualityChecks": prompt.get("qualityChecks"),
                },
                "chatRequests": [],
            },
            "response": {
                "partialEvents": [],
                "modelOutputs": [],
                "finalResult": None,
                "error": None,
            },
        }

    def add_chat_request(self, *, attempt: int, repair: bool, payload: dict[str, Any]) -> None:
        self.document["request"]["chatRequests"].append(
            {
                "attempt": attempt,
                "repair": repair,
                "payload": payload,
            }
        )

    def add_model_output(self, *, attempt: int, repair: bool, content: str) -> None:
        self.document["response"]["modelOutputs"].append(
            {
                "attempt": attempt,
                "repair": repair,
                "content": content,
            }
        )

    def add_partial_event(self, payload: dict[str, Any]) -> None:
        self.document["response"]["partialEvents"].append(payload)

    def finalize_success(self, result: dict[str, Any], repair_count: int) -> None:
        self.document["metadata"]["repairCount"] = repair_count
        self.document["metadata"]["latencyMs"] = round(
            (perf_counter() - self._started_perf) * 1000.0,
            3,
        )
        self.document["metadata"]["status"] = "success"
        self.document["response"]["finalResult"] = result
        self._write()

    def finalize_error(
        self,
        *,
        error_message: str,
        traceback_text: str,
        repair_count: int,
    ) -> None:
        self.document["metadata"]["repairCount"] = repair_count
        self.document["metadata"]["latencyMs"] = round(
            (perf_counter() - self._started_perf) * 1000.0,
            3,
        )
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
            temp_path.write_text(
                json.dumps(self.document, ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
            temp_path.replace(self.path)
        except OSError:
            return

from __future__ import annotations

import json
import os
import socket
import urllib.error
import urllib.request
from typing import Any


def _read_int_env(name: str, default: int) -> int:
    raw_value = os.environ.get(name)
    if raw_value is None:
        return default

    try:
        parsed = int(raw_value)
    except ValueError:
        return default

    return parsed if parsed > 0 else default


MODEL_DISCOVERY_TIMEOUT_SECONDS = _read_int_env(
    "RANGE_TRANSLATOR_OLLAMA_TAGS_TIMEOUT_SECONDS",
    15,
)
CHAT_TIMEOUT_SECONDS = _read_int_env(
    "RANGE_TRANSLATOR_OLLAMA_CHAT_TIMEOUT_SECONDS",
    60,
)
KEEP_ALIVE = os.environ.get("RANGE_TRANSLATOR_OLLAMA_KEEP_ALIVE", "30m")


class OllamaProvider:
    id = "ollama"
    label = "Ollama"

    def descriptor(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "label": self.label,
            "kind": "ai",
            "available": True,
            "detail": None,
        }

    def build_chat_payload(
        self,
        model: str,
        system_prompt: str,
        user_prompt: str,
        *,
        temperature: float,
        top_p: float,
    ) -> dict[str, Any]:
        request_payload = {
            "model": model,
            "stream": False,
            "format": "json",
            "keep_alive": KEEP_ALIVE,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt,
                },
                {
                    "role": "user",
                    "content": user_prompt,
                },
            ],
            "options": {
                "temperature": temperature,
                "top_p": top_p,
            },
        }
        if model.lower().startswith("qwen3"):
            request_payload["think"] = False
        return request_payload

    def resolve_model(self, endpoint: str, current_model: str) -> str:
        preferred = [
            "qwen3:8b",
            "qwen2.5:7b-instruct",
            "phi4:14b",
            "gemma3:12b",
            "mistral-nemo:12b",
            "mistral-small3.2:latest",
            "llama3.1:8b",
        ]

        try:
            request = urllib.request.Request(
                f"{endpoint}/api/tags",
                method="GET",
            )
            with urllib.request.urlopen(
                request,
                timeout=MODEL_DISCOVERY_TIMEOUT_SECONDS,
            ) as response:
                payload = json.loads(response.read().decode("utf-8"))
            model_names = [item.get("name") for item in payload.get("models", []) if item.get("name")]
        except Exception:
            model_names = []

        if current_model and current_model != "discovering" and current_model in model_names:
            return current_model

        for candidate in preferred:
            if candidate in model_names:
                return candidate

        if model_names:
            return str(model_names[0])

        return current_model if current_model else "qwen3:8b"

    def chat_json(self, endpoint: str, payload: dict[str, Any]) -> str:
        request = urllib.request.Request(
            f"{endpoint}/api/chat",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        try:
            with urllib.request.urlopen(request, timeout=CHAT_TIMEOUT_SECONDS) as response:
                body = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"Ollama returned HTTP {error.code}: {detail}") from error
        except (TimeoutError, socket.timeout) as error:
            model = payload.get("model") or "unknown"
            raise RuntimeError(
                "Ollama inference did not produce response headers within "
                f"{CHAT_TIMEOUT_SECONDS}s for model '{model}' at {endpoint}. "
                "The endpoint may still answer /api/tags while generation is stalled server-side."
            ) from error
        except urllib.error.URLError as error:
            raise RuntimeError(f"Failed to reach Ollama endpoint: {error}") from error

        message = body.get("message") or {}
        return str(message.get("content") or "")

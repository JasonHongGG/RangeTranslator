from __future__ import annotations

import json
import os
import socket
import urllib.error
import urllib.request
from typing import Any

from range_translator_runtime.core import Config
from ..ai_provider import AIProvider, GenerateRequest, GenerateResponse

class OllamaProvider(AIProvider):
    def __init__(self, model: str | None = None) -> None:
        self._model = model or "qwen3:8b"
        self._endpoint = Config.get_ollama_url().rstrip("/")
        self._chat_timeout = Config.get_ollama_timeout()
        self._keep_alive = Config.get_ollama_keep_alive()

    @property
    def name(self) -> str:
        return "ollama"

    def generate(self, request: GenerateRequest) -> GenerateResponse:
        model = self._model
        endpoint = self._endpoint

        messages = []
        if request.system_prompt:
            messages.append({"role": "system", "content": request.system_prompt})
        messages.append({"role": "user", "content": request.prompt})
        
        payload = {
            "model": model,
            "format": "json",
            "stream": False,
            "keep_alive": self._keep_alive,
            "messages": messages,
            "options": {
                "temperature": request.temperature if request.temperature is not None else 0.7,
            },
        }

        if request.max_tokens is not None:
            payload["options"]["num_predict"] = request.max_tokens

        if model.lower().startswith("qwen3"):
            payload["think"] = False

        http_request = urllib.request.Request(
            f"{endpoint}/api/chat",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        try:
            with urllib.request.urlopen(http_request, timeout=self._chat_timeout) as response:
                body = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"Ollama returned HTTP {error.code}: {detail}") from error
        except (TimeoutError, socket.timeout) as error:
            raise RuntimeError(
                f"Ollama inference did not produce response headers within "
                f"{self._chat_timeout}s for model '{model}' at {endpoint}."
            ) from error
        except urllib.error.URLError as error:
            raise RuntimeError(f"Failed to reach Ollama endpoint: {error}") from error

        message = body.get("message") or {}
        text = str(message.get("content") or "")
        
        usage = {
            "promptTokens": body.get("prompt_eval_count") or 0,
            "completionTokens": body.get("eval_count") or 0,
            "totalTokens": (body.get("prompt_eval_count") or 0) + (body.get("eval_count") or 0),
        }
        
        return GenerateResponse(
            text=text,
            usage=usage,
            metadata={"provider": self.name, "model": model}
        )

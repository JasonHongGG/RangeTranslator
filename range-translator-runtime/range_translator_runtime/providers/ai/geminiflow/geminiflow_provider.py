from __future__ import annotations

import json
import socket
import urllib.error
import urllib.request
from typing import Any, Iterator

from range_translator_runtime.core import Config
from ..ai_provider import AIProvider, GenerateRequest, GenerateResponse, GenerateStreamChunk

class GeminiFlowProvider(AIProvider):
    def __init__(self, model: str | None = None) -> None:
        self._model = model or "gemini-3-pro"
        self._endpoint = Config.get_geminiflow_url().rstrip("/")
        self._chat_timeout = 60

    @property
    def name(self) -> str:
        return "geminiflow"

    def generate(self, request: GenerateRequest) -> GenerateResponse:
        payload: dict[str, Any] = {
            "prompt": request.prompt,
            "model": self._model,
            "language": "zh-TW",
            "save_images": False,
        }
        if request.system_prompt:
            payload["system_prompt"] = request.system_prompt
        if request.images and len(request.images) > 0:
            payload["images"] = request.images
        if request.session_id:
            payload["session_id"] = request.session_id

        http_request = urllib.request.Request(
            f"{self._endpoint}/chat",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        try:
            with urllib.request.urlopen(http_request, timeout=self._chat_timeout) as response:
                body = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"GeminiFlow returned HTTP {error.code}: {detail}") from error
        except (TimeoutError, socket.timeout) as error:
            raise RuntimeError(
                f"GeminiFlow inference did not produce response headers within "
                f"{self._chat_timeout}s for model '{self._model}' at {self._endpoint}."
            ) from error
        except urllib.error.URLError as error:
            raise RuntimeError(f"Failed to reach GeminiFlow endpoint: {error}") from error

        text = str(body.get("text") or "")
        returned_images = body.get("images") or []
        
        return GenerateResponse(
            text=text,
            metadata={"provider": self.name, "model": self._model, "images": returned_images}
        )

    def generate_stream(self, request: GenerateRequest) -> Iterator[GenerateStreamChunk]:
        payload: dict[str, Any] = {
            "prompt": request.prompt,
            "model": self._model,
            "language": "zh-TW",
            "save_images": False,
        }
        if request.system_prompt:
            payload["system_prompt"] = request.system_prompt
        if request.images and len(request.images) > 0:
            payload["images"] = request.images
        if request.session_id:
            payload["session_id"] = request.session_id

        http_request = urllib.request.Request(
            f"{self._endpoint}/stream",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        try:
            with urllib.request.urlopen(http_request, timeout=self._chat_timeout) as response:
                for line in response:
                    decoded_line = line.decode("utf-8").strip()
                    if not decoded_line:
                        continue
                    if decoded_line.startswith("event:"):
                        continue
                    if decoded_line.startswith("data:"):
                        data_str = decoded_line[5:].strip()
                        if not data_str:
                            continue
                        try:
                            data = json.loads(data_str)
                            text_chunk = str(data.get("text") or "")
                            images = data.get("images") or []
                            yield GenerateStreamChunk(
                                text=text_chunk,
                                is_finished=False, # We don't have a reliable end marker in this simple SSE format unless we parse the last chunk differently, let's assume False and rely on caller
                                metadata={"provider": self.name, "model": self._model, "images": images}
                            )
                        except json.JSONDecodeError:
                            continue
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"GeminiFlow returned HTTP {error.code}: {detail}") from error
        except (TimeoutError, socket.timeout) as error:
            raise RuntimeError(
                f"GeminiFlow inference did not produce response headers within "
                f"{self._chat_timeout}s for model '{self._model}' at {self._endpoint}."
            ) from error
        except urllib.error.URLError as error:
            raise RuntimeError(f"Failed to reach GeminiFlow endpoint: {error}") from error

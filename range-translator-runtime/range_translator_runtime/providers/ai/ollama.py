from __future__ import annotations

import json
import os
import socket
import urllib.error
import urllib.request
from typing import Any

from range_translator_runtime.prompts import pretty_language, render_template
from range_translator_runtime.runtime_types import EventEmitter, PromptPayload


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

    def translate(
        self,
        payload: dict[str, Any],
        prompt: PromptPayload,
        emit_event: EventEmitter | None = None,
    ) -> dict[str, Any]:
        texts = list(payload.get("texts") or [])
        source_language = payload.get("sourceLanguage") or "auto"
        target_language = payload.get("targetLanguage") or "zh-TW"
        endpoint = str(payload.get("endpoint") or "http://127.0.0.1:11434").rstrip("/")

        if not texts:
            return {
                "providerId": self.id,
                "model": str(payload.get("model") or "discovering"),
                "promptProfile": prompt["id"],
                "detectedSource": source_language,
                "translations": [],
                "confidences": [],
            }

        model = self._resolve_model(endpoint, str(payload.get("model") or "discovering"))

        output_schema = prompt.get(
            "outputSchema",
            '{"detectedSource":"ja-JP","translations":[{"translation":"translated text","confidence":0.96}]}',
        )
        all_texts_json = json.dumps(texts, ensure_ascii=False)

        rendered_prompt = render_template(
            prompt["userTemplate"],
            {
                "source_language": source_language,
                "target_language": target_language,
                "source_language_name": pretty_language(source_language),
                "target_language_name": pretty_language(target_language),
                "line_index": "0",
                "line_number": "1",
                "line_count": str(len(texts)),
                "current_text_json": json.dumps(texts[0], ensure_ascii=False),
                "all_texts_json": all_texts_json,
                "output_schema": output_schema,
            },
        )

        content = self._chat_json(
            endpoint,
            {
                "model": model,
                "stream": False,
                "format": "json",
                "keep_alive": KEEP_ALIVE,
                "messages": [
                    {
                        "role": "system",
                        "content": prompt["system"],
                    },
                    {
                        "role": "user",
                        "content": rendered_prompt,
                    },
                ],
                "options": {
                    "temperature": 0.1,
                    "top_p": 0.9,
                },
            },
        )

        detected_source, translations, confidences = self._extract_translation_batch(
            content,
            texts,
            source_language,
        )

        if emit_event is not None:
            for index, translation in enumerate(translations):
                emit_event(
                    "translation_partial",
                    {
                        "index": index,
                        "providerId": self.id,
                        "model": model,
                        "promptProfile": prompt["id"],
                        "detectedSource": detected_source,
                        "translatedText": translation,
                        "confidence": confidences[index],
                        "done": index == len(translations) - 1,
                    },
                )

        return {
            "providerId": self.id,
            "model": model,
            "promptProfile": prompt["id"],
            "detectedSource": detected_source,
            "translations": translations,
            "confidences": confidences,
        }

    def _resolve_model(self, endpoint: str, current_model: str) -> str:
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

    def _chat_json(self, endpoint: str, payload: dict[str, Any]) -> str:
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

    def _extract_translation_batch(
        self,
        raw_content: str,
        texts: list[str],
        fallback_source: str,
    ) -> tuple[str, list[str], list[float]]:
        envelope = self._extract_translation_envelope(raw_content)
        detected_source = str(
            envelope.get("detectedSource")
            or envelope.get("detected_source")
            or fallback_source
        )
        raw_translations = (
            envelope.get("translations")
            or envelope.get("items")
            or envelope.get("lines")
            or envelope.get("results")
        )

        if isinstance(raw_translations, list):
            translations: list[str] = []
            confidences: list[float] = []
            for index, text in enumerate(texts):
                item = raw_translations[index] if index < len(raw_translations) else None
                translation, confidence = self._normalize_translation_item(item, text)
                translations.append(translation)
                confidences.append(confidence)
            return detected_source, translations, confidences

        if len(texts) == 1:
            translation, confidence = self._normalize_translation_item(envelope, texts[0])
            return detected_source, [translation], [confidence]

        raise RuntimeError("AI provider did not return a translations array")

    def _normalize_translation_item(
        self,
        item: Any,
        fallback_text: str,
    ) -> tuple[str, float]:
        if isinstance(item, dict):
            translation = (
                item.get("translation")
                or item.get("text")
                or item.get("value")
                or fallback_text
            )
            confidence = item.get("confidence")
        elif item is None:
            translation = fallback_text
            confidence = None
        else:
            translation = item
            confidence = None

        try:
            confidence_value = float(confidence if confidence is not None else 1.0)
        except (TypeError, ValueError):
            confidence_value = 1.0

        return str(translation), max(0.0, min(confidence_value, 1.0))

    def _extract_translation_envelope(self, raw_content: str) -> dict[str, Any]:
        content = raw_content.strip()
        if not content.startswith("{"):
            start = content.find("{")
            end = content.rfind("}")
            if start == -1 or end == -1:
                raise RuntimeError("AI provider did not return a JSON object")
            content = content[start : end + 1]

        return json.loads(content)

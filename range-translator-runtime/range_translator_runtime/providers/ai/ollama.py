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
        items = self._normalize_source_items(list(payload.get("items") or []))
        expected_item_count = int(payload.get("expectedItemCount") or len(items))
        source_language = payload.get("sourceLanguage") or "auto"
        target_language = payload.get("targetLanguage") or "zh-TW"
        endpoint = str(payload.get("endpoint") or "http://127.0.0.1:11434").rstrip("/")

        if expected_item_count != len(items):
            raise RuntimeError(
                f"AI request expected {expected_item_count} items but received {len(items)} source items"
            )

        if not items:
            return {
                "providerId": self.id,
                "model": str(payload.get("model") or "discovering"),
                "promptProfile": prompt["id"],
                "detectedSource": source_language,
                "items": [],
            }

        model = self._resolve_model(endpoint, str(payload.get("model") or "discovering"))

        output_schema = prompt.get(
            "outputSchema",
            '{"detectedSource":"ja-JP","items":[{"id":"source-0","index":0,"translation":"translated text","confidence":0.96}]}',
        )
        all_items_json = json.dumps(items, ensure_ascii=False)
        context_text = str(
            payload.get("contextText")
            or "\n".join(str(item["text"]) for item in items)
        )

        rendered_prompt = render_template(
            prompt["userTemplate"],
            {
                "source_language": source_language,
                "target_language": target_language,
                "source_language_name": pretty_language(source_language),
                "target_language_name": pretty_language(target_language),
                "line_index": "0",
                "line_number": "1",
                "line_count": str(len(items)),
                "item_count": str(len(items)),
                "expected_item_count": str(expected_item_count),
                "current_text_json": json.dumps(items[0]["text"], ensure_ascii=False),
                "all_items_json": all_items_json,
                "context_text_json": json.dumps(context_text, ensure_ascii=False),
                "output_schema": output_schema,
            },
        )

        request_payload = self._build_chat_payload(model, prompt["system"], rendered_prompt)
        content = self._chat_json(
            endpoint,
            request_payload,
        )

        try:
            detected_source, translated_items = self._extract_translation_batch(
                content,
                items,
                source_language,
            )
        except RuntimeError as error:
            repair_prompt = self._render_repair_prompt(
                items,
                source_language,
                target_language,
                output_schema,
                content,
                str(error),
            )
            repaired_content = self._chat_json(
                endpoint,
                self._build_chat_payload(model, prompt["system"], repair_prompt),
            )
            detected_source, translated_items = self._extract_translation_batch(
                repaired_content,
                items,
                source_language,
            )

        if emit_event is not None:
            for index, item in enumerate(translated_items):
                emit_event(
                    "translation_partial",
                    {
                        "sourceId": item["id"],
                        "index": item["index"],
                        "providerId": self.id,
                        "model": model,
                        "promptProfile": prompt["id"],
                        "detectedSource": detected_source,
                        "translatedText": item["translation"],
                        "confidence": item["confidence"],
                        "done": index == len(translated_items) - 1,
                    },
                )

        return {
            "providerId": self.id,
            "model": model,
            "promptProfile": prompt["id"],
            "detectedSource": detected_source,
            "items": translated_items,
        }

    def _build_chat_payload(self, model: str, system_prompt: str, user_prompt: str) -> dict[str, Any]:
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
                "temperature": 0.1,
                "top_p": 0.9,
            },
        }
        if model.lower().startswith("qwen3"):
            request_payload["think"] = False
        return request_payload

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
        source_items: list[dict[str, Any]],
        source_language_hint: str,
    ) -> tuple[str, list[dict[str, Any]]]:
        envelope = self._extract_translation_envelope(raw_content)
        detected_source = str(
            envelope.get("detectedSource")
            or envelope.get("detected_source")
            or source_language_hint
        )
        raw_items = envelope.get("items")
        if not isinstance(raw_items, list):
            raise RuntimeError("AI provider did not return an items array")

        if len(raw_items) != len(source_items):
            raise RuntimeError(
                f"AI provider returned {len(raw_items)} items for {len(source_items)} source items"
            )

        expected = {
            (str(item["id"]), int(item["index"])): item for item in source_items
        }
        expected_sequence = [
            (str(item["id"]), int(item["index"])) for item in source_items
        ]
        returned_sequence: list[tuple[str, int]] = []
        seen: set[tuple[str, int]] = set()
        translated_items: list[dict[str, Any]] = []

        for raw_item in raw_items:
            if not isinstance(raw_item, dict):
                raise RuntimeError("AI provider returned a non-object item")

            item_id = raw_item.get("id")
            index = raw_item.get("index")
            if not isinstance(item_id, str) or item_id == "":
                raise RuntimeError("AI provider item is missing id")
            try:
                normalized_index = int(index)
            except (TypeError, ValueError) as error:
                raise RuntimeError("AI provider item is missing a numeric index") from error

            key = (item_id, normalized_index)
            if key not in expected:
                raise RuntimeError(f"AI provider returned unexpected item id/index: {key}")
            if key in seen:
                raise RuntimeError(f"AI provider returned duplicate item id/index: {key}")
            seen.add(key)
            returned_sequence.append(key)

            translation = raw_item.get("translation")
            if not isinstance(translation, str):
                raise RuntimeError(f"AI provider item {item_id} is missing translation text")

            translated_items.append(
                {
                    "id": item_id,
                    "index": normalized_index,
                    "translation": translation.strip(),
                    "confidence": self._normalize_confidence(raw_item.get("confidence")),
                }
            )

        missing = sorted(set(expected) - seen, key=lambda item: item[1])
        if missing:
            raise RuntimeError(f"AI provider omitted item id/index pairs: {missing}")

        if returned_sequence != expected_sequence:
            raise RuntimeError("AI provider returned items out of order")

        return detected_source, translated_items

    def _normalize_source_items(self, raw_items: list[Any]) -> list[dict[str, Any]]:
        items: list[dict[str, Any]] = []
        seen: set[tuple[str, int]] = set()
        for raw_item in raw_items:
            if not isinstance(raw_item, dict):
                raise RuntimeError("AI request item must be an object")
            item_id = raw_item.get("id")
            if not isinstance(item_id, str) or item_id == "":
                raise RuntimeError("AI request item is missing id")
            try:
                index = int(raw_item.get("index"))
            except (TypeError, ValueError) as error:
                raise RuntimeError(f"AI request item {item_id} is missing numeric index") from error
            if (item_id, index) in seen:
                raise RuntimeError(f"AI request contains duplicate item id/index: {(item_id, index)}")
            seen.add((item_id, index))
            text = raw_item.get("text")
            if not isinstance(text, str):
                raise RuntimeError(f"AI request item {item_id} is missing text")
            items.append(
                {
                    "id": item_id,
                    "index": index,
                    "text": text,
                    "rect": raw_item.get("rect") or {},
                }
            )

        items.sort(key=lambda item: item["index"])
        return items

    def _normalize_confidence(self, value: Any) -> float:
        try:
            confidence_value = float(value if value is not None else 1.0)
        except (TypeError, ValueError):
            confidence_value = 1.0
        return max(0.0, min(confidence_value, 1.0))

    def _render_repair_prompt(
        self,
        source_items: list[dict[str, Any]],
        source_language: str,
        target_language: str,
        output_schema: str,
        invalid_content: str,
        validation_error: str,
    ) -> str:
        return (
            "The previous response was invalid for a realtime OCR overlay translation. "
            "Return JSON only and do not add commentary.\n"
            f"Source language: {pretty_language(source_language)} ({source_language}).\n"
            f"Target language: {pretty_language(target_language)} ({target_language}).\n"
            f"Validation error: {validation_error}\n"
            f"Expected source items JSON: {json.dumps(source_items, ensure_ascii=False)}\n"
            f"Invalid previous response: {invalid_content}\n"
            f"Required schema: {output_schema}\n"
            "Rules: return exactly one item per source item; preserve every id and index exactly; "
            "translate each item independently while using the whole list as context; never merge, split, "
            "drop, reorder, or replace source ids."
        )

    def _extract_translation_envelope(self, raw_content: str) -> dict[str, Any]:
        content = raw_content.strip()
        if not content.startswith("{"):
            start = content.find("{")
            end = content.rfind("}")
            if start == -1 or end == -1:
                raise RuntimeError("AI provider did not return a JSON object")
            content = content[start : end + 1]

        return json.loads(content)

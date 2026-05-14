from __future__ import annotations

import json
import urllib.error
import urllib.request
from typing import Any

from range_translator_runtime.prompts import pretty_language, render_template
from range_translator_runtime.runtime_types import EventEmitter, PromptPayload


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
        model = self._resolve_model(endpoint, str(payload.get("model") or "discovering"))
        detected_source = source_language
        translations: list[str] = []
        confidences: list[float] = []

        output_schema = prompt.get(
            "outputSchema",
            '{"detectedSource":"ja-JP","translation":"translated text","confidence":0.96}',
        )
        all_texts_json = json.dumps(texts, ensure_ascii=False)

        for index, text in enumerate(texts):
            rendered_prompt = render_template(
                prompt["userTemplate"],
                {
                    "source_language": source_language,
                    "target_language": target_language,
                    "source_language_name": pretty_language(source_language),
                    "target_language_name": pretty_language(target_language),
                    "line_index": str(index),
                    "line_number": str(index + 1),
                    "line_count": str(len(texts)),
                    "current_text_json": json.dumps(text, ensure_ascii=False),
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

            envelope = self._extract_translation_envelope(content)
            translation = envelope.get("translation") or text
            detected_source = envelope.get("detectedSource") or envelope.get("detected_source") or detected_source
            confidence = float(envelope.get("confidence") or 1.0)
            translations.append(str(translation))
            confidences.append(max(0.0, min(confidence, 1.0)))

            if emit_event is not None:
                emit_event(
                    "translation_partial",
                    {
                        "index": index,
                        "providerId": self.id,
                        "model": model,
                        "promptProfile": prompt["id"],
                        "detectedSource": detected_source,
                        "translatedText": str(translation),
                        "confidence": confidence,
                        "done": index == len(texts) - 1,
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
            "llama3.1:8b",
            "phi4:14b",
        ]

        try:
            request = urllib.request.Request(
                f"{endpoint}/api/tags",
                method="GET",
            )
            with urllib.request.urlopen(request, timeout=15) as response:
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
            with urllib.request.urlopen(request, timeout=35) as response:
                body = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"Ollama returned HTTP {error.code}: {detail}") from error
        except urllib.error.URLError as error:
            raise RuntimeError(f"Failed to reach Ollama endpoint: {error}") from error

        message = body.get("message") or {}
        return str(message.get("content") or "")

    def _extract_translation_envelope(self, raw_content: str) -> dict[str, Any]:
        content = raw_content.strip()
        if not content.startswith("{"):
            start = content.find("{")
            end = content.rfind("}")
            if start == -1 or end == -1:
                raise RuntimeError("AI provider did not return a JSON object")
            content = content[start : end + 1]

        return json.loads(content)

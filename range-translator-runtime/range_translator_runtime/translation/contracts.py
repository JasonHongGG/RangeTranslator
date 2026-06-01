from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from range_translator_runtime.runtime_types import PromptPayload


@dataclass(frozen=True)
class TranslationSourceItem:
    id: str
    index: int
    text: str
    rect: dict[str, Any]

    def as_prompt_item(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "index": self.index,
            "text": self.text,
            "rect": self.rect,
        }


@dataclass(frozen=True)
class TranslationRequest:
    endpoint: str
    provider_id: str
    model: str
    prompt_profile_id: str
    source_language: str
    target_language: str
    expected_item_count: int
    items: tuple[TranslationSourceItem, ...]

    @classmethod
    def from_payload(cls, payload: dict[str, Any]) -> "TranslationRequest":
        items = tuple(_normalize_source_items(list(payload.get("items") or [])))
        expected_item_count = int(payload.get("expectedItemCount") or len(items))
        if expected_item_count != len(items):
            raise RuntimeError(
                f"AI request expected {expected_item_count} items but received {len(items)} source items"
            )

        return cls(
            endpoint=str(payload.get("endpoint") or "http://127.0.0.1:11434").rstrip("/"),
            provider_id=str(payload.get("providerId") or "ollama"),
            model=str(payload.get("model") or "discovering"),
            prompt_profile_id=str(
                payload.get("promptProfile") or "translation.ui_overlay.default"
            ),
            source_language=str(payload.get("sourceLanguage") or "auto"),
            target_language=str(payload.get("targetLanguage") or "zh-TW"),
            expected_item_count=expected_item_count,
            items=items,
        )


@dataclass(frozen=True)
class TranslationPromptProfile:
    id: str
    label: str
    version: str
    task: str
    provider_family: str
    system_prompt: str
    task_context: str
    translation_template: str
    repair_template: str
    output_schema_hint: str
    style_directives: tuple[str, ...]
    quality_checks: tuple[str, ...]
    temperature: float
    top_p: float

    @classmethod
    def from_payload(cls, payload: PromptPayload) -> "TranslationPromptProfile":
        sampling = payload.get("sampling") if isinstance(payload.get("sampling"), dict) else {}
        return cls(
            id=_require_string(payload, "id"),
            label=_require_string(payload, "label"),
            version=_require_string(payload, "version"),
            task=_require_string(payload, "task"),
            provider_family=_require_string(payload, "providerFamily"),
            system_prompt=_require_string(payload, "systemPrompt"),
            task_context=_require_string(payload, "taskContext"),
            translation_template=_require_string(payload, "translationTemplate"),
            repair_template=_require_string(payload, "repairTemplate"),
            output_schema_hint=_require_string(payload, "outputSchema"),
            style_directives=tuple(_require_string_list(payload, "styleDirectives")),
            quality_checks=tuple(_require_string_list(payload, "qualityChecks")),
            temperature=_normalized_sampling_value(sampling.get("temperature"), 0.18),
            top_p=_normalized_sampling_value(sampling.get("topP"), 0.92),
        )

    def as_log_payload(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "label": self.label,
            "version": self.version,
            "task": self.task,
            "providerFamily": self.provider_family,
            "systemPrompt": self.system_prompt,
            "translationTemplate": self.translation_template,
            "repairTemplate": self.repair_template,
            "outputSchema": self.output_schema_hint,
            "styleDirectives": list(self.style_directives),
            "qualityChecks": list(self.quality_checks),
            "sampling": {
                "temperature": self.temperature,
                "topP": self.top_p,
            },
        }


@dataclass(frozen=True)
class TranslatedItem:
    id: str
    index: int
    translation: str
    confidence: float

    def as_payload(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "index": self.index,
            "translation": self.translation,
            "confidence": self.confidence,
        }


@dataclass(frozen=True)
class TranslationResult:
    provider_id: str
    model: str
    prompt_profile: str
    detected_source: str
    items: tuple[TranslatedItem, ...]

    def as_payload(self) -> dict[str, Any]:
        return {
            "providerId": self.provider_id,
            "model": self.model,
            "promptProfile": self.prompt_profile,
            "detectedSource": self.detected_source,
            "items": [item.as_payload() for item in self.items],
        }


def _normalize_source_items(raw_items: list[Any]) -> list[TranslationSourceItem]:
    items: list[TranslationSourceItem] = []
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
            raise RuntimeError(
                f"AI request item {item_id} is missing numeric index"
            ) from error

        if (item_id, index) in seen:
            raise RuntimeError(
                f"AI request contains duplicate item id/index: {(item_id, index)}"
            )
        seen.add((item_id, index))

        text = raw_item.get("text")
        if not isinstance(text, str):
            raise RuntimeError(f"AI request item {item_id} is missing text")

        raw_rect = raw_item.get("rect")
        rect = raw_rect if isinstance(raw_rect, dict) else {}

        items.append(
            TranslationSourceItem(
                id=item_id,
                index=index,
                text=text,
                rect=rect,
            )
        )

    items.sort(key=lambda item: item.index)
    return items


def _require_string(payload: PromptPayload, field_name: str) -> str:
    value = payload.get(field_name)
    if not isinstance(value, str) or not value.strip():
        raise RuntimeError(f"Prompt profile field '{field_name}' must be a non-empty string")
    return value.strip()


def _require_string_list(payload: PromptPayload, field_name: str) -> list[str]:
    value = payload.get(field_name)
    if not isinstance(value, list) or not value:
        raise RuntimeError(f"Prompt profile field '{field_name}' must be a non-empty array")

    items: list[str] = []
    for entry in value:
        if not isinstance(entry, str) or not entry.strip():
            raise RuntimeError(
                f"Prompt profile field '{field_name}' must contain non-empty strings"
            )
        items.append(entry.strip())
    return items


def _normalized_sampling_value(raw_value: Any, default: float) -> float:
    try:
        value = float(raw_value)
    except (TypeError, ValueError):
        return default
    return max(0.0, min(value, 1.0))
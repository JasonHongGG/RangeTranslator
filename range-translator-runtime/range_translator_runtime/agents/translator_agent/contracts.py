from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from range_translator_runtime.core import JsonMap

@dataclass(frozen=True)
class TranslationSourceItem:
    id: str
    index: int
    text: str
    rect: JsonMap

    def as_prompt_item(self) -> JsonMap:
        return {
            "id": self.id,
            "index": self.index,
            "text": self.text,
            "rect": self.rect,
        }

@dataclass(frozen=True)
class TranslationRequest:
    source_language: str
    target_language: str
    expected_item_count: int
    items: tuple[TranslationSourceItem, ...]

    @classmethod
    def from_payload(cls, payload: JsonMap) -> "TranslationRequest":
        items = tuple(_normalize_source_items(list(payload.get("items") or [])))
        expected_item_count = int(payload.get("expectedItemCount") or len(items))
        if expected_item_count != len(items):
            raise RuntimeError(
                f"AI request expected {expected_item_count} items but received {len(items)} source items"
            )

        return cls(
            source_language=str(payload.get("sourceLanguage") or "auto"),
            target_language=str(payload.get("targetLanguage") or "zh-TW"),
            expected_item_count=expected_item_count,
            items=items,
        )

@dataclass(frozen=True)
class TranslatedItem:
    id: str
    index: int
    translation: str
    confidence: float

    def as_payload(self) -> JsonMap:
        return {
            "id": self.id,
            "index": self.index,
            "translation": self.translation,
            "confidence": self.confidence,
        }

@dataclass(frozen=True)
class TranslationResult:
    provider_id: str
    detected_source: str
    items: tuple[TranslatedItem, ...]

    def as_payload(self) -> JsonMap:
        return {
            "providerId": self.provider_id,
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
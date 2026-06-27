from __future__ import annotations

import json
import re

from .contracts import TranslatedItem, TranslationSourceItem


def extract_translation_batch(
    raw_content: str,
    source_items: tuple[TranslationSourceItem, ...],
    source_language_hint: str,
    target_language: str,
) -> tuple[str, list[TranslatedItem]]:
    envelope = _extract_translation_envelope(raw_content)
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

    expected = {(item.id, item.index): item for item in source_items}
    expected_by_index = {item.index: item for item in source_items}
    expected_sequence = [(item.id, item.index) for item in source_items]
    returned_sequence: list[tuple[str, int]] = []
    seen: set[tuple[str, int]] = set()
    translated_items: list[TranslatedItem] = []

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

        expected_item = expected_by_index.get(normalized_index)
        if expected_item is None:
            raise RuntimeError(
                f"AI provider returned unexpected item index: {normalized_index}"
            )

        resolved_item_id = _resolve_returned_item_id(item_id, normalized_index, expected_item)
        key = (resolved_item_id, normalized_index)
        if key not in expected:
            raise RuntimeError(
                f"AI provider returned unexpected item id/index: {(item_id, normalized_index)}"
            )
        if key in seen:
            raise RuntimeError(f"AI provider returned duplicate item id/index: {key}")

        seen.add(key)
        returned_sequence.append(key)

        translation = raw_item.get("translation")
        if not isinstance(translation, str):
            raise RuntimeError(f"AI provider item {item_id} is missing translation text")

        translated_items.append(
            TranslatedItem(
                id=resolved_item_id,
                index=normalized_index,
                translation=translation.strip(),
                confidence=_normalize_confidence(raw_item.get("confidence")),
            )
        )

    missing = sorted(set(expected) - seen, key=lambda item: item[1])
    if missing:
        raise RuntimeError(f"AI provider omitted item id/index pairs: {missing}")

    if returned_sequence != expected_sequence:
        raise RuntimeError("AI provider returned items out of order")

    _validate_translated_items(
        source_items,
        translated_items,
        source_language_hint,
        target_language,
    )

    return detected_source, translated_items


def _normalize_confidence(value: object) -> float:
    try:
        confidence_value = float(value if value is not None else 1.0)
    except (TypeError, ValueError):
        confidence_value = 1.0
    return max(0.0, min(confidence_value, 1.0))


def _resolve_returned_item_id(
    item_id: str,
    normalized_index: int,
    expected_item: TranslationSourceItem,
) -> str:
    if item_id == expected_item.id:
        return expected_item.id

    if _is_legacy_index_alias(item_id, normalized_index):
        return expected_item.id

    return item_id


def _is_legacy_index_alias(item_id: str, normalized_index: int) -> bool:
    if item_id in {f"source-{normalized_index}", f"span-{normalized_index}"}:
        return True
    return item_id.endswith(f"/span-{normalized_index}")


def _validate_translated_items(
    source_items: tuple[TranslationSourceItem, ...],
    translated_items: list[TranslatedItem],
    source_language: str,
    target_language: str,
) -> None:
    for source_item, translated_item in zip(source_items, translated_items):
        translation = translated_item.translation

        if _has_immediate_phrase_repeat(translation):
            raise RuntimeError(
                f"AI provider item {translated_item.id} contains an immediate repeated phrase"
            )

        if _looks_like_repeated_source_leak(translation, source_item.text):
            raise RuntimeError(
                f"AI provider item {translated_item.id} repeated the source text instead of translating it"
            )

        if _looks_like_untranslated_ui_label(
            source_item.text,
            translation,
            source_language,
            target_language,
        ):
            raise RuntimeError(
                f"AI provider item {translated_item.id} kept the source wording instead of translating it"
            )


def _looks_like_untranslated_ui_label(
    source_text: str,
    translation: str,
    source_language: str,
    target_language: str,
) -> bool:
    if not source_text.strip() or not translation.strip():
        return False

    if source_language == target_language:
        return False

    if _compact_phrase_probe(source_text) != _compact_phrase_probe(translation):
        return False

    if _is_target_script_present(translation, target_language):
        return False

    return _looks_like_translatable_ui_label(source_text)


def _is_target_script_present(text: str, target_language: str) -> bool:
    if target_language.startswith("zh") or target_language.startswith("ja") or target_language.startswith("ko"):
        return any(_is_cjk_like(char) for char in text)
    return False


def _looks_like_translatable_ui_label(text: str) -> bool:
    stripped = text.strip()
    if not stripped:
        return False
    if _looks_like_protected_token(stripped):
        return False
    if not re.search(r"[A-Za-z]", stripped):
        return False
    if len(stripped) > 48:
        return False
    return True


def _looks_like_protected_token(text: str) -> bool:
    compact = text.strip()
    if re.search(r"https?://|www\.|\\|/|[A-Za-z]:", compact):
        return True
    if re.fullmatch(r"[A-Z0-9._+-]{2,12}", compact):
        return True
    if re.fullmatch(r"[A-Za-z]+\+[A-Za-z0-9+]+", compact):
        return True
    if re.fullmatch(r"v?\d+(?:\.\d+)+", compact):
        return True
    return False


def _looks_like_repeated_source_leak(translation: str, source_text: str) -> bool:
    normalized_translation = _compact_phrase_probe(translation)
    normalized_source = _compact_phrase_probe(source_text)
    return bool(
        normalized_source
        and normalized_translation
        and normalized_translation == normalized_source * 2
    )


def _has_immediate_phrase_repeat(value: str) -> bool:
    compact = _compact_phrase_probe(value)
    if len(compact) < 8:
        return False

    max_span = min(len(compact) // 2, 24)
    for span in range(max_span, 3, -1):
        for start in range(0, len(compact) - (span * 2) + 1):
            left = compact[start : start + span]
            right = compact[start + span : start + (span * 2)]
            if left == right and len(set(left)) > 1:
                return True

    tokens = [token for token in re.split(r"\s+", value.strip()) if token]
    if len(tokens) < 2:
        return False

    max_token_span = min(len(tokens) // 2, 4)
    for span in range(max_token_span, 0, -1):
        for start in range(0, len(tokens) - (span * 2) + 1):
            left = tokens[start : start + span]
            right = tokens[start + span : start + (span * 2)]
            if left == right and len("".join(left)) >= 6:
                return True

    return False


def _compact_phrase_probe(value: str) -> str:
    return re.sub(r"[\s\u3000,，.。;；:：!?！？()（）\[\]{}'\"`]+", "", value)


def _is_cjk_like(char: str) -> bool:
    return bool(re.match(r"[\u3400-\u4dbf\u4e00-\u9fff\u3040-\u30ff\uac00-\ud7af]", char))


def _extract_translation_envelope(raw_content: str) -> dict[str, object]:
    content = raw_content.strip()
    if not content.startswith("{"):
        start = content.find("{")
        end = content.rfind("}")
        if start == -1 or end == -1:
            raise RuntimeError("AI provider did not return a JSON object")
        content = content[start : end + 1]

    return json.loads(content)
from __future__ import annotations

import json

from range_translator_runtime.prompts import pretty_language, render_template

from .contracts import TranslationPromptProfile, TranslationRequest, TranslationSourceItem


def build_output_schema(
    source_items: tuple[TranslationSourceItem, ...],
    schema_hint: str,
) -> str:
    detected_source = "ja-JP"
    try:
        parsed_hint = json.loads(schema_hint)
    except json.JSONDecodeError:
        parsed_hint = None

    if isinstance(parsed_hint, dict):
        raw_detected = parsed_hint.get("detectedSource") or parsed_hint.get("detected_source")
        if isinstance(raw_detected, str) and raw_detected:
            detected_source = raw_detected

    example_item = source_items[0] if source_items else TranslationSourceItem(
        id="<preserve-input-id>",
        index=0,
        text="",
        rect={},
    )
    return json.dumps(
        {
            "detectedSource": detected_source,
            "items": [
                {
                    "id": example_item.id,
                    "index": example_item.index,
                    "translation": "translated text",
                    "confidence": 0.96,
                }
            ],
        },
        ensure_ascii=False,
    )


def render_translation_prompt(
    profile: TranslationPromptProfile,
    request: TranslationRequest,
    output_schema: str,
) -> str:
    return render_template(
        profile.translation_template,
        _render_values(profile, request, output_schema, validation_error=""),
    )


def render_repair_prompt(
    profile: TranslationPromptProfile,
    request: TranslationRequest,
    output_schema: str,
    validation_error: str,
) -> str:
    return render_template(
        profile.repair_template,
        _render_values(profile, request, output_schema, validation_error=validation_error),
    )


def _render_values(
    profile: TranslationPromptProfile,
    request: TranslationRequest,
    output_schema: str,
    validation_error: str,
) -> dict[str, str]:
    all_items_json = json.dumps(
        [item.as_prompt_item() for item in request.items],
        ensure_ascii=False,
    )
    first_item_text = request.items[0].text if request.items else ""

    return {
        "task_context": profile.task_context,
        "source_language": request.source_language,
        "target_language": request.target_language,
        "source_language_name": pretty_language(request.source_language),
        "target_language_name": pretty_language(request.target_language),
        "expected_item_count": str(request.expected_item_count),
        "item_count": str(len(request.items)),
        "line_count": str(len(request.items)),
        "current_text_json": json.dumps(first_item_text, ensure_ascii=False),
        "all_items_json": all_items_json,
        "output_schema": output_schema,
        "style_rules": _render_rule_block(profile.style_directives),
        "quality_rules": _render_rule_block(profile.quality_checks),
        "validation_error": validation_error,
    }


def _render_rule_block(rules: tuple[str, ...]) -> str:
    return "\n".join(f"- {rule}" for rule in rules)
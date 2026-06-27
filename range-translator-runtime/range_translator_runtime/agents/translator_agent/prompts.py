from __future__ import annotations

import json
from typing import Any

from range_translator_runtime.core import JsonMap
from .contracts import TranslationRequest, TranslationSourceItem

def build_output_schema(source_items: tuple[TranslationSourceItem, ...]) -> str:
    example_item = source_items[0] if source_items else TranslationSourceItem(
        id="<preserve-input-id>",
        index=0,
        text="",
        rect={},
    )
    return json.dumps(
        {
            "detectedSource": "ja-JP",
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

def build_system_prompt() -> str:
    return (
        "You translate OCR text captured from a live desktop overlay into polished, natural UI wording. "
        "Return JSON only. Do not add markdown, commentary, or explanatory notes."
    )

def build_user_prompt(request: TranslationRequest, output_schema: str, validation_error: str = "") -> str:
    source_items_json = json.dumps([item.as_prompt_item() for item in request.items], ensure_ascii=False)
    item_count = len(request.items)
    
    prompt = (
        f"Translate ordered OCR source items for a realtime desktop overlay.\n"
        f"Source language hint: {request.source_language}. Target language: {request.target_language}.\n"
        f"Expected item count: {request.expected_item_count}.\n"
        "Task context: Translate short desktop UI labels, menus, toggles, button text, tooltips, and status strings "
        "so the user can understand them instantly without mentally reconstructing a literal translation.\n\n"
    )

    if validation_error:
        prompt += (
            "The previous response failed realtime overlay translation validation.\n"
            f"Validation error:\n{validation_error}\n\n"
            "Repair rules:\n"
            "- Fix the structure first.\n"
            "- Preserve every id and index exactly.\n"
            "- Return exactly one item per source item.\n"
            "- Keep translations concise and natural for desktop UI.\n"
            "- Do not echo or discuss the invalid response.\n\n"
        )

    prompt += (
        "OCR source items JSON:\n"
        f"{source_items_json}\n\n"
        "Style directives:\n"
        "- Write the translation the way a native Traditional Chinese software UI would present it.\n"
        "- Prefer polished, immediately understandable wording over literal calques.\n"
        "- Keep labels concise, but do not make them cryptic.\n"
        "- Recover the most plausible UI intent when OCR is slightly noisy, as long as the surrounding item list supports that reading.\n"
        "- Preserve tone consistency across related menu items and settings labels.\n\n"
        "Quality checks:\n"
        "- Every source item must have exactly one aligned output item with the same id and index.\n"
        "- Do not leave a normal English UI label untranslated unless it is clearly a protected token or product name.\n"
        "- Avoid awkward literal translations that force the user to infer the intended UI meaning.\n"
        "- Avoid duplicate phrases, filler wording, and explanatory suffixes.\n"
        "- Keep wording short enough for overlay rendering while preserving the original UI intent.\n\n"
        "Return JSON only with this exact schema:\n"
        f"{output_schema}\n\n"
        "Hard rules:\n"
        "- Return exactly one output item for every input item.\n"
        "- Preserve each item's id and index exactly.\n"
        "- Do not merge, split, drop, reorder, or invent items.\n"
        "- Keep each translation concise enough to fit the original UI slot.\n"
        "- Use the whole ordered list as context, but each output item must still map only to its original OCR position.\n"
        "- Prefer natural Traditional Chinese UI phrasing over literal word-by-word translation.\n"
        "- Keep shortcuts, numbers, URLs, file paths, IDs, hotkeys, and product names unchanged when appropriate.\n"
        "- If an item is already in the target language, keep it.\n"
        "- If an item text is empty, return an empty translation for the same id and index.\n"
        "- Never add notes.\n"
        "- Never repeat a phrase twice in immediate succession.\n"
        "- Confidence must be a float between 0 and 1 reflecting how reliable each translation is under OCR uncertainty."
    )
    
    return prompt
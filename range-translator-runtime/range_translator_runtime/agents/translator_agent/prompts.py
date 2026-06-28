from __future__ import annotations

import json
from typing import Any

from range_translator_runtime.core import JsonMap
from .contracts import TranslationRequest, TranslationSourceItem

def build_system_prompt() -> str:
    return (
        "You translate OCR text captured from a live desktop overlay into polished, natural UI wording. "
        "Return JSON only. Do not add markdown, commentary, or explanatory notes."
    )

def build_user_prompt(request: TranslationRequest, validation_error: str = "") -> str:
    source_items_json = json.dumps([item.as_prompt_item() for item in request.items], ensure_ascii=False)
    
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
        "### JSON Format Explanation\n"
        "You will receive an array of input items. Each item represents a piece of text recognized on the screen.\n"
        "Input fields:\n"
        "- `id`: A unique identifier for the text element. You must return this exactly as provided.\n"
        "- `index`: The sequential order of the text element. You must return this exactly as provided.\n"
        "- `text`: The original OCR text to be translated.\n"
        "- `rect`: The bounding box coordinates of the text. (Context only, do not return).\n\n"
        "You must output a JSON object containing `detectedSource` and `items`.\n"
        "Output fields for each item:\n"
        "- `id`: Must match the input `id`.\n"
        "- `index`: Must match the input `index`.\n"
        "- `translation`: The translated text in the target language.\n"
        "- `confidence`: A float between 0 and 1 indicating your confidence in the translation accuracy.\n\n"
        "### Example Input\n"
        "[\n"
        '  {"id": "span-0", "index": 0, "text": "Settings", "rect": {"x": 10, "y": 20, "width": 50, "height": 15}},\n'
        '  {"id": "span-1", "index": 1, "text": "Apply", "rect": {"x": 10, "y": 50, "width": 40, "height": 15}}\n'
        "]\n\n"
        "### Example Output\n"
        "{\n"
        '  "detectedSource": "en-US",\n'
        '  "items": [\n'
        '    {"id": "span-0", "index": 0, "translation": "設定", "confidence": 0.98},\n'
        '    {"id": "span-1", "index": 1, "translation": "套用", "confidence": 0.99}\n'
        "  ]\n"
        "}\n\n"
        "### Input Data\n"
        "OCR source items JSON:\n"
        f"{source_items_json}\n\n"
        "### Style Directives\n"
        "- Write the translation the way a native software UI would present it.\n"
        "- Prefer polished, immediately understandable wording over literal calques.\n"
        "- Keep labels concise, but do not make them cryptic.\n"
        "- Recover the most plausible UI intent when OCR is slightly noisy, as long as the surrounding item list supports that reading.\n"
        "- Preserve tone consistency across related menu items and settings labels.\n\n"
        "### Quality Checks\n"
        "- Every source item must have exactly one aligned output item with the same id and index.\n"
        "- Do not leave a normal English UI label untranslated unless it is clearly a protected token or product name.\n"
        "- Avoid awkward literal translations that force the user to infer the intended UI meaning.\n"
        "- Avoid duplicate phrases, filler wording, and explanatory suffixes.\n"
        "- Keep wording short enough for overlay rendering while preserving the original UI intent.\n\n"
        "### Hard Rules\n"
        "- Return JSON only.\n"
        "- Return exactly one output item for every input item.\n"
        "- Preserve each item's `id` and `index` exactly.\n"
        "- Do not merge, split, drop, reorder, or invent items.\n"
        "- Keep each translation concise enough to fit the original UI slot.\n"
        "- Prefer natural UI phrasing over literal word-by-word translation.\n"
        "- Keep shortcuts, numbers, URLs, file paths, IDs, hotkeys, and product names unchanged when appropriate.\n"
        "- If an item is already in the target language, keep it.\n"
        "- If an item text is empty, return an empty translation for the same id and index.\n"
        "- Never add notes, markdown code blocks, or commentary outside the JSON.\n"
        "- Never repeat a phrase twice in immediate succession.\n"
        "- Confidence must be a float between 0 and 1 reflecting how reliable each translation is under OCR uncertainty."
    )
    
    return prompt
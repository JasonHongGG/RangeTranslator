from __future__ import annotations


def render_template(template: str, values: dict[str, str]) -> str:
    rendered = template
    for key, value in values.items():
        rendered = rendered.replace(f"{{{{{key}}}}}", value)
    return rendered


def pretty_language(tag: str) -> str:
    mapping = {
        "zh-TW": "Traditional Chinese",
        "zh-Hans": "Simplified Chinese",
        "ja-JP": "Japanese",
        "ko-KR": "Korean",
        "fr-FR": "French",
        "de-DE": "German",
        "es-ES": "Spanish",
        "ru-RU": "Russian",
        "th-TH": "Thai",
        "vi-VN": "Vietnamese",
        "id-ID": "Indonesian",
        "en-US": "English",
        "auto": "Automatic Detection",
    }
    return mapping.get(tag, tag or "English")

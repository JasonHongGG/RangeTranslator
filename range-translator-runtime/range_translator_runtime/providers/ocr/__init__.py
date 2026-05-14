from .base import OcrProvider


def build_ocr_providers() -> dict[str, OcrProvider]:
    return {}


__all__ = ["OcrProvider", "build_ocr_providers"]

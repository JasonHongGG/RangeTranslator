from .base import OcrProvider
from .paddleocr_provider import PaddleOcrProvider


def build_ocr_providers() -> dict[str, OcrProvider]:
    provider = PaddleOcrProvider()
    return {provider.id: provider}


__all__ = ["OcrProvider", "PaddleOcrProvider", "build_ocr_providers"]

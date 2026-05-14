from __future__ import annotations

import base64
import ctypes
import io
import platform
from dataclasses import dataclass
from typing import Any


@dataclass(slots=True)
class _RuntimeSupport:
    available: bool
    detail: str | None
    paddle_ocr: Any | None
    numpy: Any | None
    pil_image: Any | None
    use_gpu: bool


class PaddleOcrProvider:
    id = "paddleocr"
    label = "PaddleOCR"

    def __init__(self) -> None:
        self._support = self._load_support()
        self._runtime_detail = self._support.detail
        self._engines: dict[str, Any] = {}

    def descriptor(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "label": self.label,
            "kind": "ocr",
            "available": self._support.available,
            "detail": self._runtime_detail,
        }

    def recognize(self, payload: dict[str, Any]) -> dict[str, Any]:
        if not self._support.available:
            raise RuntimeError(self._support.detail or "PaddleOCR is unavailable")

        image_base64 = str(payload.get("imagePngBase64") or "")
        if not image_base64:
            raise RuntimeError("OCR request is missing imagePngBase64")

        requested_source = str(payload.get("sourceLanguage") or "auto")
        hint_language = payload.get("hintLanguage")
        hint_language = str(hint_language) if hint_language else None

        image = self._decode_image(image_base64)
        candidate_groups = self._candidate_groups(requested_source, hint_language)

        best_language = "und"
        best_lines: list[dict[str, Any]] = []
        best_score = -1.0

        for group in candidate_groups:
            engine = self._get_engine(group)
            lines = self._run_engine(engine, group.lang, image)
            score = self._score(lines, group)
            if score > best_score:
                best_score = score
                best_language = group.resolved_tag
                best_lines = lines

        return {
            "providerId": self.id,
            "language": best_language,
            "lines": best_lines,
        }

    def _load_support(self) -> _RuntimeSupport:
        try:
            import numpy as np
            from PIL import Image
        except Exception as error:
            return _RuntimeSupport(
                available=False,
                detail=f"Missing OCR image dependencies: {error}",
                paddle_ocr=None,
                numpy=None,
                pil_image=None,
                use_gpu=False,
            )

        try:
            import paddle
        except Exception as error:
            return _RuntimeSupport(
                available=False,
                detail=(
                    "Paddle is not installed. Install a matching paddlepaddle or "
                    f"paddlepaddle-gpu package first: {error}"
                ),
                paddle_ocr=None,
                numpy=np,
                pil_image=Image,
                use_gpu=False,
            )

        try:
            from paddleocr import PaddleOCR
        except Exception as error:
            return _RuntimeSupport(
                available=False,
                detail=f"PaddleOCR import failed: {error}",
                paddle_ocr=None,
                numpy=np,
                pil_image=Image,
                use_gpu=False,
            )

        use_gpu = bool(getattr(paddle.device, "is_compiled_with_cuda", lambda: False)())
        if not use_gpu:
            return _RuntimeSupport(
                available=False,
                detail="Installed Paddle build is CPU-only. This runtime requires a supported GPU build.",
                paddle_ocr=PaddleOCR,
                numpy=np,
                pil_image=Image,
                use_gpu=False,
            )

        if not _has_required_cuda_runtime():
            return _RuntimeSupport(
                available=False,
                detail="Missing CUDA/cuDNN runtime for Paddle GPU.",
                paddle_ocr=PaddleOCR,
                numpy=np,
                pil_image=Image,
                use_gpu=False,
            )

        unsupported_detail = _unsupported_gpu_detail(paddle)
        if unsupported_detail is not None:
            return _RuntimeSupport(
                available=False,
                detail=unsupported_detail,
                paddle_ocr=PaddleOCR,
                numpy=np,
                pil_image=Image,
                use_gpu=False,
            )

        device_name = _gpu_device_name(paddle)
        detail = f"GPU ready: {device_name}" if device_name else "GPU ready"
        return _RuntimeSupport(
            available=True,
            detail=detail,
            paddle_ocr=PaddleOCR,
            numpy=np,
            pil_image=Image,
            use_gpu=use_gpu,
        )

    def _decode_image(self, image_base64: str) -> Any:
        payload = base64.b64decode(image_base64.encode("utf-8"))
        with self._support.pil_image.open(io.BytesIO(payload)) as image:
            return self._support.numpy.array(image.convert("RGB"))

    def _candidate_groups(
        self,
        requested_source: str,
        hint_language: str | None,
    ) -> list["_LanguageGroup"]:
        explicit_group = _map_language_group(requested_source)
        if requested_source.lower() != "auto" and explicit_group is not None:
            return [explicit_group]

        groups: list[_LanguageGroup] = []

        for candidate in [hint_language, "ja-JP", "zh-TW", "ko-KR", "en-US"]:
            if not candidate:
                continue
            group = _map_language_group(candidate)
            if group is None:
                continue
            if not any(existing.lang == group.lang for existing in groups):
                groups.append(group)

        if not groups:
            groups.append(_LanguageGroup(lang="en", resolved_tag="en-US"))

        return groups

    def _get_engine(self, group: "_LanguageGroup") -> Any:
        cache_key = self._engine_cache_key(group.lang, self._support.use_gpu)
        if cache_key not in self._engines:
            self._engines[cache_key] = self._support.paddle_ocr(
                use_angle_cls=False,
                lang=group.lang,
                show_log=False,
                use_gpu=self._support.use_gpu,
            )
        return self._engines[cache_key]

    def _run_engine(self, engine: Any, lang: str, image: Any) -> list[dict[str, Any]]:
        try:
            raw_result = engine.ocr(image, cls=False)
        except RuntimeError as error:
            raise RuntimeError(f"PaddleOCR GPU inference failed: {error}") from error
        return self._parse_result(raw_result)

    def _parse_result(self, raw_result: Any) -> list[dict[str, Any]]:
        if not raw_result:
            return []

        entries = raw_result[0] if isinstance(raw_result, list) else raw_result
        if not entries:
            return []

        lines: list[dict[str, Any]] = []
        for line in entries:
            if not line or len(line) < 2:
                continue
            points, content = line[0], line[1]
            if not isinstance(content, (list, tuple)) or len(content) < 2:
                continue
            text = str(content[0]).strip()
            if not text:
                continue
            confidence = float(content[1])
            rect = _rect_from_points(points)
            lines.append(
                {
                    "text": text,
                    "rect": rect,
                    "confidence": max(0.0, min(confidence, 1.0)),
                }
            )
        return lines

    def _score(self, lines: list[dict[str, Any]], group: "_LanguageGroup") -> float:
        text = "".join(str(line.get("text") or "") for line in lines)
        base_score = sum(
            len(str(line.get("text") or "").replace(" ", "")) * float(line.get("confidence") or 0.0)
            for line in lines
        )
        return base_score + _script_bonus(text, group.lang)

    def _engine_cache_key(self, lang: str, use_gpu: bool) -> str:
        return f"{'gpu' if use_gpu else 'cpu'}:{lang}"


@dataclass(frozen=True, slots=True)
class _LanguageGroup:
    lang: str
    resolved_tag: str


def _map_language_group(tag: str) -> _LanguageGroup | None:
    normalized = tag.lower()
    if normalized == "auto":
        return None
    if normalized.startswith("ja"):
        return _LanguageGroup(lang="japan", resolved_tag="ja-JP")
    if normalized.startswith("ko"):
        return _LanguageGroup(lang="korean", resolved_tag="ko-KR")
    if normalized.startswith("zh"):
        return _LanguageGroup(lang="ch", resolved_tag="zh-TW" if "tw" in normalized or "hant" in normalized else "zh-Hans")
    if normalized.startswith("en"):
        return _LanguageGroup(lang="en", resolved_tag="en-US")
    if normalized.startswith(("fr", "de", "es", "it", "pt")):
        return _LanguageGroup(lang="latin", resolved_tag=tag)
    if normalized.startswith(("ru", "uk", "bg", "sr")):
        return _LanguageGroup(lang="cyrillic", resolved_tag=tag)
    return _LanguageGroup(lang="en", resolved_tag=tag)


def _rect_from_points(points: Any) -> dict[str, int]:
    xs = [int(round(point[0])) for point in points]
    ys = [int(round(point[1])) for point in points]
    left = max(min(xs), 0)
    top = max(min(ys), 0)
    right = max(xs)
    bottom = max(ys)
    return {
        "x": left,
        "y": top,
        "width": max(right - left, 1),
        "height": max(bottom - top, 1),
    }


def _is_gpu_runtime_error(error: RuntimeError) -> bool:
    message = str(error).lower()
    return "cudnn" in message or "cuda" in message or "dynamic library" in message


def _has_required_cuda_runtime() -> bool:
    try:
        ctypes.WinDLL("cudnn64_8.dll")
        return True
    except OSError:
        return False


def _gpu_device_name(paddle: Any) -> str | None:
    try:
        return str(paddle.device.cuda.get_device_name())
    except Exception:
        return None


def _unsupported_gpu_detail(paddle: Any) -> str | None:
    try:
        properties = paddle.device.cuda.get_device_properties()
    except Exception as error:
        return f"Failed to inspect GPU properties for Paddle: {error}"

    capability = f"{properties.major}.{properties.minor}"
    version = str(getattr(paddle, "__version__", "unknown"))
    if (
        platform.system() == "Windows"
        and version.startswith("2.6.")
        and int(properties.major) >= 12
    ):
        return (
            "Installed Paddle GPU wheel "
            f"({version}) does not support GPU compute capability {capability} on Windows. "
            "This machine needs a newer official Windows GPU wheel, or a Linux/source-built runtime."
        )

    return None


def _script_bonus(text: str, lang: str) -> float:
    stripped = "".join(character for character in text if not character.isspace())
    if not stripped:
        return 0.0

    ascii_latin = sum(character.isascii() and (character.isalnum() or character in "'-_./,:;!?()[]{}") for character in stripped)
    hiragana_katakana = sum(_is_hiragana_katakana(character) for character in stripped)
    cjk = sum(_is_cjk(character) for character in stripped)
    hangul = sum(_is_hangul(character) for character in stripped)
    cyrillic = sum(_is_cyrillic(character) for character in stripped)
    total = len(stripped)

    if lang in {"en", "latin"}:
        return 12.0 if ascii_latin / total >= 0.8 and cjk == 0 and hangul == 0 and cyrillic == 0 else 0.0
    if lang == "japan":
        if hiragana_katakana > 0:
            return 12.0
        if ascii_latin / total >= 0.8:
            return -6.0
        return 2.0 if cjk > 0 else 0.0
    if lang == "ch":
        if cjk > 0 and hiragana_katakana == 0:
            return 8.0
        if ascii_latin / total >= 0.8:
            return -6.0
        return 0.0
    if lang == "korean":
        if hangul > 0:
            return 12.0
        if ascii_latin / total >= 0.8:
            return -6.0
        return 0.0
    if lang == "cyrillic":
        if cyrillic > 0:
            return 12.0
        if ascii_latin / total >= 0.8:
            return -6.0
        return 0.0
    return 0.0


def _is_hiragana_katakana(character: str) -> bool:
    codepoint = ord(character)
    return 0x3040 <= codepoint <= 0x30ff


def _is_cjk(character: str) -> bool:
    codepoint = ord(character)
    return 0x4e00 <= codepoint <= 0x9fff


def _is_hangul(character: str) -> bool:
    codepoint = ord(character)
    return 0xac00 <= codepoint <= 0xd7af


def _is_cyrillic(character: str) -> bool:
    codepoint = ord(character)
    return 0x0400 <= codepoint <= 0x04ff
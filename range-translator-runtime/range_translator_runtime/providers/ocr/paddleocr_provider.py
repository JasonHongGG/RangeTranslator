from __future__ import annotations

import base64
import contextlib
import ctypes
import io
from collections.abc import Iterable
from dataclasses import dataclass
from typing import Any


DETECTION_MODEL_NAME = "PP-OCRv5_mobile_det"
AUTO_ACCEPT_SCORE = 8.0


@dataclass(frozen=True, slots=True)
class _LanguageGroup:
    paddle_lang: str
    recognition_model: str
    resolved_tag: str
    score_key: str


@dataclass(slots=True)
class _RuntimeSupport:
    available: bool
    detail: str | None
    paddle_ocr: Any | None
    paddle: Any | None
    numpy: Any | None
    pil_image: Any | None


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
            lines = self._run_engine(engine, image)
            score = self._score(lines, group)
            if score > best_score:
                best_score = score
                best_language = group.resolved_tag
                best_lines = lines
            if self._should_accept_candidate(
                lines,
                score,
                requested_source,
                hint_language,
            ):
                return {
                    "providerId": self.id,
                    "language": group.resolved_tag,
                    "lines": lines,
                }

        return {
            "providerId": self.id,
            "language": best_language,
            "lines": best_lines,
        }

    def prewarm(self, payload: dict[str, Any]) -> dict[str, Any]:
        if not self._support.available:
            raise RuntimeError(self._support.detail or "PaddleOCR is unavailable")

        requested_source = str(payload.get("sourceLanguage") or "auto")
        hint_language = payload.get("hintLanguage")
        hint_language = str(hint_language) if hint_language else None

        candidate_groups = self._candidate_groups(requested_source, hint_language)
        group = candidate_groups[0]
        self._get_engine(group)
        return {
            "providerId": self.id,
            "language": group.resolved_tag,
            "detail": self._runtime_detail or "PaddleOCR warmed",
        }

    def _should_accept_candidate(
        self,
        lines: list[dict[str, Any]],
        score: float,
        requested_source: str,
        hint_language: str | None,
    ) -> bool:
        if not lines:
            return False

        if requested_source.lower() != "auto":
            return True

        if hint_language:
            return True

        return score >= AUTO_ACCEPT_SCORE

    def _load_support(self) -> _RuntimeSupport:
        try:
            import numpy as np
            from PIL import Image
        except Exception as error:
            return _RuntimeSupport(
                available=False,
                detail=f"Missing OCR image dependencies: {error}",
                paddle_ocr=None,
                paddle=None,
                numpy=None,
                pil_image=None,
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
                paddle=None,
                numpy=np,
                pil_image=Image,
            )

        try:
            import paddleocr as paddleocr_package
            from paddleocr import PaddleOCR
        except Exception as error:
            return _RuntimeSupport(
                available=False,
                detail=f"PaddleOCR import failed: {error}",
                paddle_ocr=None,
                paddle=paddle,
                numpy=np,
                pil_image=Image,
            )

        use_gpu = bool(getattr(paddle.device, "is_compiled_with_cuda", lambda: False)())
        if not use_gpu:
            return _RuntimeSupport(
                available=False,
                detail="Installed Paddle build is CPU-only. This runtime requires a supported GPU build.",
                paddle_ocr=PaddleOCR,
                paddle=paddle,
                numpy=np,
                pil_image=Image,
            )

        if not _has_required_cuda_runtime():
            return _RuntimeSupport(
                available=False,
                detail="Missing CUDA/cuDNN runtime for Paddle GPU.",
                paddle_ocr=PaddleOCR,
                paddle=paddle,
                numpy=np,
                pil_image=Image,
            )

        validation_error = _validate_gpu_runtime(paddle)
        if validation_error is not None:
            return _RuntimeSupport(
                available=False,
                detail=validation_error,
                paddle_ocr=PaddleOCR,
                paddle=paddle,
                numpy=np,
                pil_image=Image,
            )

        device_name = _gpu_device_name(paddle)
        paddleocr_version = getattr(paddleocr_package, "__version__", "unknown")
        detail_parts = [
            f"GPU ready: {device_name}" if device_name else "GPU ready",
            f"Paddle {getattr(paddle, '__version__', 'unknown')}",
            f"PaddleOCR {paddleocr_version}",
            f"CUDA {paddle.version.cuda()}",
        ]
        return _RuntimeSupport(
            available=True,
            detail=" | ".join(detail_parts),
            paddle_ocr=PaddleOCR,
            paddle=paddle,
            numpy=np,
            pil_image=Image,
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

        family_fallbacks = [
            "ja-JP",
            "en-US",
            "ko-KR",
            "zh-Hans",
            "fr-FR",
            "ru-RU",
            "th-TH",
        ]

        candidates = []
        if hint_language:
            candidates.append(hint_language)
        candidates.extend(family_fallbacks)

        seen_candidates: set[tuple[str, str]] = set()
        for candidate in candidates:
            if not candidate:
                continue
            group = _map_language_group(candidate)
            if group is None:
                continue
            cache_key = (group.recognition_model, group.score_key)
            if cache_key in seen_candidates:
                continue
            seen_candidates.add(cache_key)
            groups.append(group)

        if not groups:
            groups.append(_map_language_group("en-US"))

        return groups

    def _get_engine(self, group: "_LanguageGroup") -> Any:
        cache_key = self._engine_cache_key(group.recognition_model)
        if cache_key not in self._engines:
            self._engines[cache_key] = self._support.paddle_ocr(
                text_detection_model_name=DETECTION_MODEL_NAME,
                text_recognition_model_name=group.recognition_model,
                use_doc_orientation_classify=False,
                use_doc_unwarping=False,
                use_textline_orientation=False,
                device="gpu:0",
            )
        return self._engines[cache_key]

    def _run_engine(self, engine: Any, image: Any) -> list[dict[str, Any]]:
        try:
            raw_result = engine.predict(image)
        except Exception as error:
            raise RuntimeError(f"PaddleOCR PP-OCRv5 inference failed: {error}") from error
        return self._parse_result(raw_result)

    def _parse_result(self, raw_result: Any) -> list[dict[str, Any]]:
        if not raw_result:
            return []

        page = raw_result[0] if isinstance(raw_result, list) else raw_result
        if not page:
            return []

        rec_texts = list(page.get("rec_texts") or [])
        rec_scores = list(page.get("rec_scores") or [])
        rec_boxes = _normalize_sequence(page.get("rec_boxes"))
        rec_polys = _normalize_sequence(page.get("rec_polys"))
        dt_polys = _normalize_sequence(page.get("dt_polys"))

        lines: list[dict[str, Any]] = []
        for index, content in enumerate(rec_texts):
            text = str(content or "").strip()
            if not text:
                continue

            confidence = 0.0
            if index < len(rec_scores):
                confidence = float(rec_scores[index])

            rect = None
            if index < len(rec_boxes):
                rect = _rect_from_box(rec_boxes[index])
            elif index < len(rec_polys):
                rect = _rect_from_points(rec_polys[index])
            elif index < len(dt_polys):
                rect = _rect_from_points(dt_polys[index])

            if rect is None:
                continue

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
        return base_score + _script_bonus(text, group.score_key)

    def _engine_cache_key(self, recognition_model: str) -> str:
        return recognition_model


def _map_language_group(tag: str) -> _LanguageGroup | None:
    normalized = tag.lower()
    if normalized == "auto":
        return None
    if normalized.startswith("ja"):
        return _LanguageGroup(
            paddle_lang="japan",
            recognition_model="PP-OCRv5_mobile_rec",
            resolved_tag="ja-JP",
            score_key="japan",
        )
    if normalized.startswith("ko"):
        return _LanguageGroup(
            paddle_lang="korean",
            recognition_model="korean_PP-OCRv5_mobile_rec",
            resolved_tag="ko-KR",
            score_key="korean",
        )
    if normalized.startswith("zh"):
        return _LanguageGroup(
            paddle_lang="chinese_cht" if "tw" in normalized or "hant" in normalized else "ch",
            recognition_model="PP-OCRv5_mobile_rec",
            resolved_tag="zh-TW" if "tw" in normalized or "hant" in normalized else "zh-Hans",
            score_key="ch",
        )
    if normalized.startswith("en"):
        return _LanguageGroup(
            paddle_lang="en",
            recognition_model="en_PP-OCRv5_mobile_rec",
            resolved_tag="en-US",
            score_key="en",
        )
    if normalized.startswith(("fr", "de", "es", "it", "pt", "vi", "id")):
        resolved_tag = _canonical_tag(tag)
        return _LanguageGroup(
            paddle_lang=resolved_tag.split("-")[0].lower(),
            recognition_model="latin_PP-OCRv5_mobile_rec",
            resolved_tag=resolved_tag,
            score_key="latin",
        )
    if normalized.startswith(("ru", "uk", "bg", "sr")):
        resolved_tag = _canonical_tag(tag)
        return _LanguageGroup(
            paddle_lang=resolved_tag.split("-")[0].lower(),
            recognition_model="eslav_PP-OCRv5_mobile_rec",
            resolved_tag=resolved_tag,
            score_key="cyrillic",
        )
    if normalized.startswith("th"):
        return _LanguageGroup(
            paddle_lang="th",
            recognition_model="th_PP-OCRv5_mobile_rec",
            resolved_tag="th-TH",
            score_key="thai",
        )
    return _LanguageGroup(
        paddle_lang="en",
        recognition_model="en_PP-OCRv5_mobile_rec",
        resolved_tag=_canonical_tag(tag),
        score_key="en",
    )


def _canonical_tag(tag: str) -> str:
    normalized = tag.lower()
    aliases = {
        "fr": "fr-FR",
        "de": "de-DE",
        "es": "es-ES",
        "ru": "ru-RU",
        "th": "th-TH",
        "vi": "vi-VN",
        "id": "id-ID",
        "uk": "uk-UA",
        "bg": "bg-BG",
        "sr": "sr-RS",
    }
    return aliases.get(normalized, tag)


def _normalize_sequence(value: Any) -> list[Any]:
    if value is None:
        return []
    if hasattr(value, "tolist"):
        value = value.tolist()
    if isinstance(value, list):
        return value
    if isinstance(value, tuple):
        return list(value)
    return []


def _rect_from_box(box: Any) -> dict[str, int] | None:
    if not isinstance(box, Iterable):
        return None
    values = [int(round(float(point))) for point in box]
    if len(values) != 4:
        return None
    left, top, right, bottom = values
    return {
        "x": max(left, 0),
        "y": max(top, 0),
        "width": max(right - left, 1),
        "height": max(bottom - top, 1),
    }


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


def _has_required_cuda_runtime() -> bool:
    for library_name in ("cudnn64_9.dll", "cudnn64_8.dll"):
        try:
            ctypes.WinDLL(library_name)
            return True
        except OSError:
            continue
    return False


def _gpu_device_name(paddle: Any) -> str | None:
    try:
        return str(paddle.device.cuda.get_device_name())
    except Exception:
        return None


def _validate_gpu_runtime(paddle: Any) -> str | None:
    stdout_buffer = io.StringIO()
    stderr_buffer = io.StringIO()

    try:
        with contextlib.redirect_stdout(stdout_buffer), contextlib.redirect_stderr(
            stderr_buffer
        ):
            paddle.utils.run_check()
    except Exception as error:
        details: list[str] = []
        stdout_output = stdout_buffer.getvalue().strip()
        stderr_output = stderr_buffer.getvalue().strip()
        if stdout_output:
            details.append(f"stdout: {stdout_output}")
        if stderr_output:
            details.append(f"stderr: {stderr_output}")

        detail_suffix = f" ({' | '.join(details)})" if details else ""
        return f"Paddle GPU validation failed: {error}{detail_suffix}"
    return None


def _script_bonus(text: str, lang: str) -> float:
    stripped = "".join(character for character in text if not character.isspace())
    if not stripped:
        return 0.0

    ascii_latin = sum(character.isascii() and (character.isalnum() or character in "'-_./,:;!?()[]{}") for character in stripped)
    latin_extended = sum(_is_latin_extended(character) for character in stripped)
    hiragana_katakana = sum(_is_hiragana_katakana(character) for character in stripped)
    cjk = sum(_is_cjk(character) for character in stripped)
    hangul = sum(_is_hangul(character) for character in stripped)
    cyrillic = sum(_is_cyrillic(character) for character in stripped)
    total = len(stripped)

    if lang == "en":
        if latin_extended > 0:
            return -4.0
        return 14.0 if ascii_latin / total >= 0.8 and cjk == 0 and hangul == 0 and cyrillic == 0 else 0.0
    if lang == "latin":
        if latin_extended > 0:
            return 12.0
        if ascii_latin / total >= 0.8 and cjk == 0 and hangul == 0 and cyrillic == 0:
            return 4.0
        return 0.0
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
    if lang == "thai":
        thai = sum(_is_thai(character) for character in stripped)
        if thai > 0:
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


def _is_latin_extended(character: str) -> bool:
    codepoint = ord(character)
    return 0x00c0 <= codepoint <= 0x024f


def _is_thai(character: str) -> bool:
    codepoint = ord(character)
    return 0x0e00 <= codepoint <= 0x0e7f


def _is_cyrillic(character: str) -> bool:
    codepoint = ord(character)
    return 0x0400 <= codepoint <= 0x04ff
from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from range_translator_runtime.providers.ai.ollama import OllamaProvider
from range_translator_runtime.translation import (
    TranslationOrchestrator,
    TranslationPromptProfile,
    TranslationRequest,
)
from range_translator_runtime.translation.prompting import (
    build_output_schema,
    render_repair_prompt,
)
from range_translator_runtime.translation.validation import extract_translation_batch


PROMPT = {
    "id": "translation.ui_overlay.default",
    "label": "UI Overlay Translation Polished",
    "version": "2.0.0",
    "task": "desktop-ui-translation",
    "providerFamily": "ollama-chat",
    "systemPrompt": "Return JSON only.",
    "taskContext": "Translate desktop UI copy into polished Traditional Chinese.",
    "translationTemplate": "Items: {{all_items_json}}\nStyle:\n{{style_rules}}\nQuality:\n{{quality_rules}}\nSchema: {{output_schema}}",
    "repairTemplate": "Validation error: {{validation_error}}\nItems: {{all_items_json}}\nSchema: {{output_schema}}",
    "styleDirectives": [
        "Prefer natural Traditional Chinese UI wording.",
        "Keep labels concise.",
    ],
    "qualityChecks": [
        "Keep id and index aligned.",
        "Do not leave ordinary English UI labels untranslated.",
    ],
    "sampling": {"temperature": 0.18, "topP": 0.92},
    "outputSchema": '{"detectedSource":"ja-JP","items":[{"id":"<preserve-input-id>","index":0,"translation":"translated text","confidence":0.96}]}',
}


def source_items() -> list[dict[str, object]]:
    return [
        {
            "id": "7:1/span-0",
            "index": 0,
            "text": "Settings Layering Restored",
            "rect": {"x": 10, "y": 20, "width": 200, "height": 28},
        },
        {
            "id": "7:1/span-1",
            "index": 1,
            "text": "Synchronized Pinning",
            "rect": {"x": 10, "y": 56, "width": 180, "height": 28},
        },
    ]


def long_source_items() -> list[dict[str, object]]:
    return [
        {
            "id": "9:4/span-0",
            "index": 0,
            "text": "The layered overlay contract now preserves each OCR span independently.",
            "rect": {"x": 16, "y": 18, "width": 420, "height": 32},
        },
        {
            "id": "9:4/span-1",
            "index": 1,
            "text": "Long translated paragraphs must stay inside their original visual region.",
            "rect": {"x": 16, "y": 58, "width": 440, "height": 34},
        },
        {
            "id": "9:4/span-2",
            "index": 2,
            "text": "Badge colors should follow the original surface instead of the page background.",
            "rect": {"x": 16, "y": 100, "width": 460, "height": 34},
        },
    ]


class OllamaProviderAlignmentTests(unittest.TestCase):
    def setUp(self) -> None:
        self.provider = OllamaProvider()
        self.profile = TranslationPromptProfile.from_payload(PROMPT)
        self.request = TranslationRequest.from_payload(
            {
                "endpoint": "http://127.0.0.1:11434",
                "providerId": "ollama",
                "model": "qwen3:8b",
                "promptProfile": "translation.ui_overlay.default",
                "sourceLanguage": "en-US",
                "targetLanguage": "zh-TW",
                "expectedItemCount": 2,
                "items": source_items(),
            }
        )

    def test_extracts_id_aligned_items(self) -> None:
        raw = json.dumps(
            {
                "detectedSource": "en-US",
                "items": [
                    {
                        "id": "7:1/span-0",
                        "index": 0,
                        "translation": "設定層級已恢復",
                        "confidence": 0.9,
                    },
                    {
                        "id": "7:1/span-1",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                ],
            },
            ensure_ascii=False,
        )

        detected_source, items = extract_translation_batch(
            raw,
            tuple(self.request.items),
            "auto",
            "zh-TW",
        )

        self.assertEqual(detected_source, "en-US")
        self.assertEqual([item.id for item in items], ["7:1/span-0", "7:1/span-1"])
        self.assertEqual([item.translation for item in items], ["設定層級已恢復", "同步釘選"])

    def test_accepts_legacy_source_id_alias_when_index_matches(self) -> None:
        raw = json.dumps(
            {
                "detectedSource": "en-US",
                "items": [
                    {
                        "id": "source-0",
                        "index": 0,
                        "translation": "設定層級已恢復",
                        "confidence": 0.9,
                    },
                    {
                        "id": "source-1",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                ],
            },
            ensure_ascii=False,
        )

        detected_source, items = extract_translation_batch(
            raw,
            tuple(self.request.items),
            "auto",
            "zh-TW",
        )

        self.assertEqual(detected_source, "en-US")
        self.assertEqual([item.id for item in items], ["7:1/span-0", "7:1/span-1"])

    def test_rejects_merged_output(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {
                        "id": "source-0",
                        "index": 0,
                        "translation": "設定層級已恢復；同步釘選",
                        "confidence": 0.7,
                    }
                ]
            },
            ensure_ascii=False,
        )

        with self.assertRaisesRegex(RuntimeError, "returned 1 items for 2 source items"):
            extract_translation_batch(raw, tuple(self.request.items), "auto", "zh-TW")

    def test_rejects_unexpected_id(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {
                        "id": "source-0",
                        "index": 0,
                        "translation": "設定層級已恢復",
                        "confidence": 0.9,
                    },
                    {
                        "id": "wrong-id",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                ]
            },
            ensure_ascii=False,
        )

        with self.assertRaisesRegex(RuntimeError, "unexpected item id/index"):
            extract_translation_batch(raw, tuple(self.request.items), "auto", "zh-TW")

    def test_rejects_reordered_items(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {
                        "id": "7:1/span-1",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                    {
                        "id": "7:1/span-0",
                        "index": 0,
                        "translation": "設定層級已恢復",
                        "confidence": 0.9,
                    },
                ]
            },
            ensure_ascii=False,
        )

        with self.assertRaisesRegex(RuntimeError, "out of order"):
            extract_translation_batch(raw, tuple(self.request.items), "auto", "zh-TW")

    def test_empty_translation_is_not_replaced_with_source_text(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {"id": "7:1/span-0", "index": 0, "translation": "", "confidence": 0.3},
                    {"id": "7:1/span-1", "index": 1, "translation": "同步釘選", "confidence": 0.8},
                ]
            },
            ensure_ascii=False,
        )

        _, items = extract_translation_batch(raw, tuple(self.request.items), "auto", "zh-TW")

        self.assertEqual(items[0].translation, "")

    def test_rejects_immediate_repeated_phrase(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {
                        "id": "7:1/span-0",
                        "index": 0,
                        "translation": "用來更新用來更新現有的變更規格",
                        "confidence": 0.4,
                    },
                    {
                        "id": "7:1/span-1",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                ]
            },
            ensure_ascii=False,
        )

        with self.assertRaisesRegex(RuntimeError, "immediate repeated phrase"):
            extract_translation_batch(raw, tuple(self.request.items), "auto", "zh-TW")

    def test_rejects_untranslated_plain_english_ui_label(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {
                        "id": "7:1/span-0",
                        "index": 0,
                        "translation": "Settings Layering Restored",
                        "confidence": 0.8,
                    },
                    {
                        "id": "7:1/span-1",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                ]
            },
            ensure_ascii=False,
        )

        with self.assertRaisesRegex(RuntimeError, "kept the source wording"):
            extract_translation_batch(raw, tuple(self.request.items), "en-US", "zh-TW")

    def test_accepts_long_multi_span_batch_without_collapsing_order(self) -> None:
        request = TranslationRequest.from_payload(
            {
                "endpoint": "http://127.0.0.1:11434",
                "providerId": "ollama",
                "model": "qwen3:8b",
                "promptProfile": "translation.ui_overlay.default",
                "sourceLanguage": "en-US",
                "targetLanguage": "zh-TW",
                "expectedItemCount": 3,
                "items": long_source_items(),
            }
        )

        raw = json.dumps(
            {
                "detectedSource": "en-US",
                "items": [
                    {
                        "id": "9:4/span-0",
                        "index": 0,
                        "translation": "分層 overlay 契約現在會分別保留每個 OCR 區塊。",
                        "confidence": 0.88,
                    },
                    {
                        "id": "9:4/span-1",
                        "index": 1,
                        "translation": "較長的翻譯段落也必須留在原本的視覺區塊內。",
                        "confidence": 0.9,
                    },
                    {
                        "id": "9:4/span-2",
                        "index": 2,
                        "translation": "badge 的顏色應該跟隨原本表面，而不是整個頁面背景。",
                        "confidence": 0.87,
                    },
                ],
            },
            ensure_ascii=False,
        )

        _, items = extract_translation_batch(raw, tuple(request.items), "en-US", "zh-TW")

        self.assertEqual([item.id for item in items], ["9:4/span-0", "9:4/span-1", "9:4/span-2"])
        self.assertEqual(items[1].translation, "較長的翻譯段落也必須留在原本的視覺區塊內。")

    def test_repair_prompt_omits_invalid_response_echo(self) -> None:
        prompt = render_repair_prompt(
            self.profile,
            self.request,
            PROMPT["outputSchema"],
            "AI provider returned items out of order",
        )

        self.assertNotIn("設定層級已恢復；同步釘選", prompt)
        self.assertIn("Validation error", prompt)

    def test_build_output_schema_uses_actual_first_item_id(self) -> None:
        schema = build_output_schema(tuple(self.request.items), PROMPT["outputSchema"])

        self.assertIn('"id": "7:1/span-0"', schema)
        self.assertNotIn('"id": "source-0"', schema)

    def test_translate_repairs_invalid_first_response_once(self) -> None:
        class RepairingProvider(OllamaProvider):
            def __init__(self) -> None:
                super().__init__()
                self.calls = 0

            def resolve_model(self, endpoint: str, current_model: str) -> str:
                return "qwen3:8b"

            def chat_json(self, endpoint: str, payload: dict[str, object]) -> str:
                self.calls += 1
                if self.calls == 1:
                    return json.dumps(
                        {
                            "items": [
                                {
                                    "id": "source-0",
                                    "index": 0,
                                    "translation": "設定層級已恢復；同步釘選",
                                    "confidence": 0.6,
                                }
                            ]
                        },
                        ensure_ascii=False,
                    )
                return json.dumps(
                    {
                        "items": [
                            {
                                "id": "source-0",
                                "index": 0,
                                "translation": "設定層級已恢復",
                                "confidence": 0.9,
                            },
                            {
                                "id": "source-1",
                                "index": 1,
                                "translation": "同步釘選",
                                "confidence": 0.8,
                            },
                        ]
                    },
                    ensure_ascii=False,
                )

        class StubPromptRepository:
            def load(self, prompt_id: str) -> dict[str, object]:
                self.last_prompt_id = prompt_id
                return PROMPT

        provider = RepairingProvider()
        orchestrator = TranslationOrchestrator(StubPromptRepository(), {"ollama": provider})

        response = orchestrator.translate(
            {
                "endpoint": "http://127.0.0.1:11434",
                "providerId": "ollama",
                "model": "qwen3:8b",
                "promptProfile": "translation.ui_overlay.default",
                "sourceLanguage": "en-US",
                "targetLanguage": "zh-TW",
                "expectedItemCount": 2,
                "items": source_items(),
            }
        )

        self.assertEqual(provider.calls, 2)
        self.assertEqual(
            [item["translation"] for item in response["items"]],
            ["設定層級已恢復", "同步釘選"],
        )

    def test_translate_writes_ai_log_file(self) -> None:
        class LoggingProvider(OllamaProvider):
            def resolve_model(self, endpoint: str, current_model: str) -> str:
                return "qwen3:8b"

            def chat_json(self, endpoint: str, payload: dict[str, object]) -> str:
                return json.dumps(
                    {
                        "items": [
                            {
                                "id": "source-0",
                                "index": 0,
                                "translation": "設定層級已恢復",
                                "confidence": 0.9,
                            },
                            {
                                "id": "source-1",
                                "index": 1,
                                "translation": "同步釘選",
                                "confidence": 0.8,
                            },
                        ]
                    },
                    ensure_ascii=False,
                )

        class StubPromptRepository:
            def load(self, prompt_id: str) -> dict[str, object]:
                self.last_prompt_id = prompt_id
                return PROMPT

        provider = LoggingProvider()
        orchestrator = TranslationOrchestrator(StubPromptRepository(), {"ollama": provider})
        previous = os.environ.get("RANGE_TRANSLATOR_AI_LOG_DIR")

        with tempfile.TemporaryDirectory() as temp_dir:
            os.environ["RANGE_TRANSLATOR_AI_LOG_DIR"] = temp_dir
            try:
                orchestrator.translate(
                    {
                        "_rtRequestId": 42,
                        "endpoint": "http://127.0.0.1:11434",
                        "providerId": "ollama",
                        "model": "qwen3:8b",
                        "promptProfile": "translation.ui_overlay.default",
                        "sourceLanguage": "en-US",
                        "targetLanguage": "zh-TW",
                        "expectedItemCount": 2,
                        "items": source_items(),
                    }
                )
            finally:
                if previous is None:
                    os.environ.pop("RANGE_TRANSLATOR_AI_LOG_DIR", None)
                else:
                    os.environ["RANGE_TRANSLATOR_AI_LOG_DIR"] = previous

            files = list(Path(temp_dir).glob("*_translate_*.json"))
            self.assertEqual(len(files), 1)
            document = json.loads(files[0].read_text(encoding="utf-8"))
            self.assertEqual(document["metadata"]["requestId"], 42)
            self.assertEqual(document["metadata"]["status"], "success")
            self.assertEqual(document["metadata"]["itemCount"], 2)
            self.assertEqual(document["response"]["finalResult"]["providerId"], "ollama")
            self.assertEqual(len(document["request"]["chatRequests"]), 1)
            self.assertIn("translationTemplate", document["request"]["prompt"])

    def test_translate_logs_errors(self) -> None:
        class FailingProvider(OllamaProvider):
            def resolve_model(self, endpoint: str, current_model: str) -> str:
                return "qwen3:8b"

            def chat_json(self, endpoint: str, payload: dict[str, object]) -> str:
                raise RuntimeError("synthetic failure")

        class StubPromptRepository:
            def load(self, prompt_id: str) -> dict[str, object]:
                self.last_prompt_id = prompt_id
                return PROMPT

        provider = FailingProvider()
        orchestrator = TranslationOrchestrator(StubPromptRepository(), {"ollama": provider})
        previous = os.environ.get("RANGE_TRANSLATOR_AI_LOG_DIR")

        with tempfile.TemporaryDirectory() as temp_dir:
            os.environ["RANGE_TRANSLATOR_AI_LOG_DIR"] = temp_dir
            try:
                with self.assertRaisesRegex(RuntimeError, "synthetic failure"):
                    orchestrator.translate(
                        {
                            "_rtRequestId": 7,
                            "endpoint": "http://127.0.0.1:11434",
                            "providerId": "ollama",
                            "model": "qwen3:8b",
                            "promptProfile": "translation.ui_overlay.default",
                            "sourceLanguage": "en-US",
                            "targetLanguage": "zh-TW",
                            "expectedItemCount": 2,
                            "items": source_items(),
                        }
                    )
            finally:
                if previous is None:
                    os.environ.pop("RANGE_TRANSLATOR_AI_LOG_DIR", None)
                else:
                    os.environ["RANGE_TRANSLATOR_AI_LOG_DIR"] = previous

            files = list(Path(temp_dir).glob("*_translate_*.json"))
            self.assertEqual(len(files), 1)
            document = json.loads(files[0].read_text(encoding="utf-8"))
            self.assertEqual(document["metadata"]["requestId"], 7)
            self.assertEqual(document["metadata"]["status"], "error")
            self.assertIn("synthetic failure", document["response"]["error"]["message"])


if __name__ == "__main__":
    unittest.main()
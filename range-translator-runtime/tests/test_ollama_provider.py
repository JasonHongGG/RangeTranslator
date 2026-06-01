from __future__ import annotations

import json
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from range_translator_runtime.providers.ai.ollama import OllamaProvider


PROMPT = {
    "id": "translation.ui_overlay.default",
    "system": "Return JSON only.",
    "userTemplate": "Items: {{all_items_json}} Schema: {{output_schema}}",
    "outputSchema": '{"detectedSource":"ja-JP","items":[{"id":"source-0","index":0,"translation":"translated text","confidence":0.96}]}',
}


def source_items() -> list[dict[str, object]]:
    return [
        {
            "id": "source-0",
            "index": 0,
            "text": "Settings Layering Restored",
            "rect": {"x": 10, "y": 20, "width": 200, "height": 28},
        },
        {
            "id": "source-1",
            "index": 1,
            "text": "Synchronized Pinning",
            "rect": {"x": 10, "y": 56, "width": 180, "height": 28},
        },
    ]


class OllamaProviderAlignmentTests(unittest.TestCase):
    def setUp(self) -> None:
        self.provider = OllamaProvider()

    def test_extracts_id_aligned_items(self) -> None:
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

        detected_source, items = self.provider._extract_translation_batch(
            raw,
            source_items(),
            "auto",
        )

        self.assertEqual(detected_source, "en-US")
        self.assertEqual([item["id"] for item in items], ["source-0", "source-1"])
        self.assertEqual([item["translation"] for item in items], ["設定層級已恢復", "同步釘選"])

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
            self.provider._extract_translation_batch(raw, source_items(), "auto")

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
                        "id": "source-x",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                ]
            },
            ensure_ascii=False,
        )

        with self.assertRaisesRegex(RuntimeError, "unexpected item id/index"):
            self.provider._extract_translation_batch(raw, source_items(), "auto")

    def test_rejects_reordered_items(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {
                        "id": "source-1",
                        "index": 1,
                        "translation": "同步釘選",
                        "confidence": 0.8,
                    },
                    {
                        "id": "source-0",
                        "index": 0,
                        "translation": "設定層級已恢復",
                        "confidence": 0.9,
                    },
                ]
            },
            ensure_ascii=False,
        )

        with self.assertRaisesRegex(RuntimeError, "out of order"):
            self.provider._extract_translation_batch(raw, source_items(), "auto")

    def test_empty_translation_is_not_replaced_with_source_text(self) -> None:
        raw = json.dumps(
            {
                "items": [
                    {"id": "source-0", "index": 0, "translation": "", "confidence": 0.3},
                    {"id": "source-1", "index": 1, "translation": "同步釘選", "confidence": 0.8},
                ]
            },
            ensure_ascii=False,
        )

        _, items = self.provider._extract_translation_batch(raw, source_items(), "auto")

        self.assertEqual(items[0]["translation"], "")

    def test_translate_repairs_invalid_first_response_once(self) -> None:
        class RepairingProvider(OllamaProvider):
            def __init__(self) -> None:
                super().__init__()
                self.calls = 0

            def _resolve_model(self, endpoint: str, current_model: str) -> str:
                return "qwen3:8b"

            def _chat_json(self, endpoint: str, payload: dict[str, object]) -> str:
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

        provider = RepairingProvider()

        response = provider.translate(
            {
                "endpoint": "http://127.0.0.1:11434",
                "providerId": "ollama",
                "model": "qwen3:8b",
                "promptProfile": "translation.ui_overlay.default",
                "sourceLanguage": "en-US",
                "targetLanguage": "zh-TW",
                "expectedItemCount": 2,
                "contextText": "Settings Layering Restored\nSynchronized Pinning",
                "items": source_items(),
            },
            PROMPT,
        )

        self.assertEqual(provider.calls, 2)
        self.assertEqual(
            [item["translation"] for item in response["items"]],
            ["設定層級已恢復", "同步釘選"],
        )


if __name__ == "__main__":
    unittest.main()
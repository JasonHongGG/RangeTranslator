from __future__ import annotations

import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from range_translator_runtime.providers.ocr.paddleocr_provider import PaddleOcrProvider


class PaddleOcrProviderContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.provider = PaddleOcrProvider.__new__(PaddleOcrProvider)

    def test_parse_result_prefers_rec_boxes_and_preserves_order(self) -> None:
        lines = self.provider._parse_result(
            [
                {
                    "rec_texts": ["General", "Save"],
                    "rec_scores": [0.91, 1.2],
                    "rec_boxes": [
                        [10, 4, 96, 22],
                        [12, 36, 72, 54],
                    ],
                }
            ]
        )

        self.assertEqual([line["text"] for line in lines], ["General", "Save"])
        self.assertEqual(lines[0]["rect"], {"x": 10, "y": 4, "width": 86, "height": 18})
        self.assertEqual(lines[1]["rect"], {"x": 12, "y": 36, "width": 60, "height": 18})
        self.assertEqual(lines[1]["confidence"], 1.0)

    def test_parse_result_falls_back_to_polygons_when_boxes_are_missing(self) -> None:
        lines = self.provider._parse_result(
            [
                {
                    "rec_texts": ["Badge", "Layered text"],
                    "rec_scores": [0.82, 0.77],
                    "rec_polys": [
                        [[8.2, 6.1], [62.8, 6.1], [62.8, 24.3], [8.2, 24.3]],
                    ],
                    "dt_polys": [
                        [[8.2, 6.1], [62.8, 6.1], [62.8, 24.3], [8.2, 24.3]],
                        [[14.0, 42.2], [128.4, 42.2], [128.4, 63.7], [14.0, 63.7]],
                    ],
                }
            ]
        )

        self.assertEqual(lines[0]["rect"], {"x": 8, "y": 6, "width": 55, "height": 18})
        self.assertEqual(lines[1]["rect"], {"x": 14, "y": 42, "width": 114, "height": 22})
        self.assertEqual([line["text"] for line in lines], ["Badge", "Layered text"])

    def test_parse_result_discards_empty_text_and_invalid_geometry(self) -> None:
        lines = self.provider._parse_result(
            [
                {
                    "rec_texts": ["", "Visible"],
                    "rec_scores": [0.5, 0.66],
                    "rec_boxes": [
                        [0, 0, 0, 0],
                        [18, 10, 54, 28],
                    ],
                }
            ]
        )

        self.assertEqual(lines, [
            {
                "text": "Visible",
                "rect": {"x": 18, "y": 10, "width": 36, "height": 18},
                "confidence": 0.66,
            }
        ])


if __name__ == "__main__":
    unittest.main()
use crate::{
    capture::{CapturedFrame, estimate_colors},
    models::{OcrRecognitionLine, OverlaySourceUnit, PixelRect, SelectionRect, TextAlign},
};

use super::geometry::{GeometryTransformer, TextMetrics, estimate_text_metrics};

#[derive(Debug, Clone, PartialEq)]
pub struct SpanStyle {
    pub foreground: String,
    pub background: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceSpan {
    pub id: String,
    pub frame_id: String,
    pub order: usize,
    pub source_text: String,
    pub source_rect: PixelRect,
    pub text_metrics: TextMetrics,
    pub confidence: f32,
    pub style: SpanStyle,
    pub align: TextAlign,
}

impl SourceSpan {
    pub fn as_overlay_unit(&self) -> OverlaySourceUnit {
        OverlaySourceUnit {
            id: self.id.clone(),
            frame_id: self.frame_id.clone(),
            order: self.order,
            source_text: self.source_text.clone(),
            source_rect: self.source_rect.clone(),
            font_size: self.text_metrics.font_size,
            line_height: self.text_metrics.line_height,
            confidence: self.confidence,
            foreground: self.style.foreground.clone(),
            background: self.style.background.clone(),
            style_confidence: self.style.confidence,
            align: self.align,
        }
    }
}

pub struct SceneBuilder<'a> {
    frame: &'a CapturedFrame,
    transformer: GeometryTransformer<'a>,
    frame_id: &'a str,
}

impl<'a> SceneBuilder<'a> {
    pub fn new(frame: &'a CapturedFrame, selection: &'a SelectionRect, frame_id: &'a str) -> Self {
        Self {
            frame,
            transformer: GeometryTransformer::new(frame, selection),
            frame_id,
        }
    }

    pub fn build_source_spans(&self, lines: &[OcrRecognitionLine]) -> Vec<SourceSpan> {
        let mut ordered_lines = lines.iter().collect::<Vec<_>>();
        ordered_lines.sort_by_key(|line| (line.rect.y, line.rect.x));

        ordered_lines
            .into_iter()
            .enumerate()
            .map(|(order, line)| self.build_source_span(line, order))
            .collect()
    }

    fn build_source_span(&self, line: &OcrRecognitionLine, order: usize) -> SourceSpan {
        let style = estimate_colors(&self.frame.image, &line.rect);
        let normalized_rect = self.transformer.selection_rect_from_frame_rect(&line.rect);
        let text_metrics = estimate_text_metrics(&normalized_rect, &line.text);

        SourceSpan {
            id: format!("{}/span-{}", self.frame_id, order),
            frame_id: self.frame_id.to_string(),
            order,
            source_text: line.text.clone(),
            source_rect: PixelRect {
                x: normalized_rect.x,
                y: normalized_rect.y,
                width: normalized_rect.width.max(1),
                height: normalized_rect.height.max(1),
            },
            text_metrics,
            confidence: line.confidence,
            style: SpanStyle {
                foreground: style.foreground,
                background: style.background,
                confidence: style.confidence,
            },
            align: TextAlign::Left,
        }
    }
}

pub fn canonicalize_ocr_lines(lines: &[OcrRecognitionLine]) -> Vec<OcrRecognitionLine> {
    let mut canonical = Vec::new();

    for line in lines {
        let normalized_text = normalize_ocr_text(&line.text);
        if normalized_text.is_empty() {
            continue;
        }

        if let Some(existing) = canonical
            .iter_mut()
            .find(|existing| should_merge_ocr_line(existing, line))
        {
            existing.rect = merge_pixel_rect(&existing.rect, &line.rect);
            existing.confidence = existing.confidence.max(line.confidence);
            if line.text.chars().count() > existing.text.chars().count() {
                existing.text = line.text.clone();
            }
            continue;
        }

        canonical.push(line.clone());
    }

    canonical
}

fn normalize_ocr_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn should_merge_ocr_line(left: &OcrRecognitionLine, right: &OcrRecognitionLine) -> bool {
    normalize_ocr_text(&left.text) == normalize_ocr_text(&right.text)
        && rects_refer_to_same_region(&left.rect, &right.rect)
}

fn rects_refer_to_same_region(left: &PixelRect, right: &PixelRect) -> bool {
    let intersection = rect_intersection_area(left, right) as f32;
    if intersection <= 0.0 {
        return false;
    }

    let smaller_area = (left.width * left.height).min(right.width * right.height) as f32;
    let overlap_over_smaller = intersection / smaller_area.max(1.0);

    rect_iou(left, right) >= 0.72
        || overlap_over_smaller >= 0.9
        || (overlap_over_smaller >= 0.78 && rect_centers_are_close(left, right))
}

fn rect_centers_are_close(left: &PixelRect, right: &PixelRect) -> bool {
    let left_center_x = left.x as f32 + left.width as f32 / 2.0;
    let left_center_y = left.y as f32 + left.height as f32 / 2.0;
    let right_center_x = right.x as f32 + right.width as f32 / 2.0;
    let right_center_y = right.y as f32 + right.height as f32 / 2.0;

    let allowed_dx = left.width.min(right.width) as f32 * 0.18;
    let allowed_dy = left.height.min(right.height) as f32 * 0.22;

    (left_center_x - right_center_x).abs() <= allowed_dx.max(1.0)
        && (left_center_y - right_center_y).abs() <= allowed_dy.max(1.0)
}

fn rect_intersection_area(left: &PixelRect, right: &PixelRect) -> u32 {
    let x0 = left.x.max(right.x);
    let y0 = left.y.max(right.y);
    let x1 = (left.x + left.width).min(right.x + right.width);
    let y1 = (left.y + left.height).min(right.y + right.height);

    if x1 <= x0 || y1 <= y0 {
        return 0;
    }

    (x1 - x0) * (y1 - y0)
}

fn rect_iou(left: &PixelRect, right: &PixelRect) -> f32 {
    let intersection = rect_intersection_area(left, right) as f32;
    if intersection == 0.0 {
        return 0.0;
    }

    let left_area = (left.width * left.height) as f32;
    let right_area = (right.width * right.height) as f32;
    let union = left_area + right_area - intersection;
    if union <= 0.0 {
        return 0.0;
    }

    intersection / union
}

fn merge_pixel_rect(left: &PixelRect, right: &PixelRect) -> PixelRect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = (left.x + left.width).max(right.x + right.width);
    let bottom_edge = (left.y + left.height).max(right.y + right.height);

    PixelRect {
        x,
        y,
        width: (right_edge - x).max(1),
        height: (bottom_edge - y).max(1),
    }
}

use crate::{capture::CapturedFrame, models::{PixelRect, SelectionRect}};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    pub font_size: f32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GeometryTransformer<'a> {
    frame: &'a CapturedFrame,
    selection: &'a SelectionRect,
}

impl<'a> GeometryTransformer<'a> {
    pub fn new(frame: &'a CapturedFrame, selection: &'a SelectionRect) -> Self {
        Self { frame, selection }
    }

    pub fn selection_rect_from_frame_rect(&self, rect: &PixelRect) -> PixelRect {
        let frame_width = self.frame.metadata.capture_width.max(1) as f32;
        let frame_height = self.frame.metadata.capture_height.max(1) as f32;
        let selection_width = self.selection.width.max(1) as f32;
        let selection_height = self.selection.height.max(1) as f32;

        let left = ((rect.x as f32 / frame_width) * selection_width).floor();
        let top = ((rect.y as f32 / frame_height) * selection_height).floor();
        let right = ((((rect.x + rect.width) as f32) / frame_width) * selection_width).ceil();
        let bottom = ((((rect.y + rect.height) as f32) / frame_height) * selection_height).ceil();

        PixelRect {
            x: left.max(0.0) as u32,
            y: top.max(0.0) as u32,
            width: (right - left).max(1.0) as u32,
            height: (bottom - top).max(1.0) as u32,
        }
    }
}

pub fn estimate_text_metrics(rect: &PixelRect, text: &str) -> TextMetrics {
    let char_count = text.chars().count().max(1) as f32;
    let rect_height = rect.height.max(1) as f32;
    let rect_width = rect.width.max(1) as f32;
    let height_bound = (rect_height * 0.85).max(10.0);
    let width_bound = ((rect_width / char_count).max(6.0) * 1.8).max(10.0);
    let font_size = height_bound.min(width_bound).max(10.0);
    let line_height = (font_size * 1.15).min((rect_height * 1.05).max(font_size));

    TextMetrics {
        font_size,
        line_height,
    }
}

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use image::RgbaImage;
use screenshots::Screen;

use crate::models::{CaptureCoordinateSpace, CaptureMetadata, PixelRect, SelectionRect};

const FRAME_SIGNATURE_GRID: usize = 12;
const FRAME_SIGNATURE_BUCKETS: usize = FRAME_SIGNATURE_GRID * FRAME_SIGNATURE_GRID;
const FRAME_SIGNATURE_CHANGED_CELL_DELTA: u32 = 8;
const FRAME_SIGNATURE_PEAK_DELTA: u32 = 16;
const FRAME_SIGNATURE_TOTAL_DELTA: u32 = 120;
const FRAME_SIGNATURE_CHANGED_CELLS: u32 = 4;

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub image: RgbaImage,
    pub metadata: CaptureMetadata,
}

#[derive(Debug, Clone)]
pub struct DesktopBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

fn physical_origin(value: i32, scale_factor: f32) -> i32 {
    ((value as f32) * scale_factor).round() as i32
}

fn physical_size(value: u32, scale_factor: f32) -> u32 {
    ((value as f32) * scale_factor).round().max(1.0) as u32
}

fn logical_offset(value: i32, scale_factor: f32) -> i32 {
    ((value as f32) / scale_factor).round() as i32
}

fn logical_size(value: u32, scale_factor: f32) -> u32 {
    ((value as f32) / scale_factor).round().max(1.0) as u32
}

fn physical_display_bounds(screen: &Screen) -> DesktopBounds {
    let scale_factor = screen.display_info.scale_factor;
    DesktopBounds {
        x: physical_origin(screen.display_info.x, scale_factor),
        y: physical_origin(screen.display_info.y, scale_factor),
        width: physical_size(screen.display_info.width, scale_factor),
        height: physical_size(screen.display_info.height, scale_factor),
    }
}

fn selection_fits_bounds(selection: &SelectionRect, bounds: &DesktopBounds) -> bool {
    selection.x >= bounds.x
        && selection.y >= bounds.y
        && selection.x + selection.width as i32 <= bounds.x + bounds.width as i32
        && selection.y + selection.height as i32 <= bounds.y + bounds.height as i32
}

#[derive(Debug, Clone)]
pub struct FrameSignature {
    buckets: [u8; FRAME_SIGNATURE_BUCKETS],
}

impl FrameSignature {
    pub fn from_image(image: &RgbaImage) -> Self {
        let mut buckets = [0_u8; FRAME_SIGNATURE_BUCKETS];

        let width = image.width().max(1);
        let height = image.height().max(1);
        let cell_w = (width as f32 / FRAME_SIGNATURE_GRID as f32).max(1.0);
        let cell_h = (height as f32 / FRAME_SIGNATURE_GRID as f32).max(1.0);

        for row in 0..FRAME_SIGNATURE_GRID {
            for col in 0..FRAME_SIGNATURE_GRID {
                let start_x = (col as f32 * cell_w).floor() as u32;
                let end_x = (((col + 1) as f32) * cell_w).ceil() as u32;
                let start_y = (row as f32 * cell_h).floor() as u32;
                let end_y = (((row + 1) as f32) * cell_h).ceil() as u32;

                let mut total = 0_f32;
                let mut count = 0_f32;
                for y in start_y..end_y.min(height) {
                    for x in start_x..end_x.min(width) {
                        let pixel = image.get_pixel(x, y);
                        total += luminance(pixel.0[0], pixel.0[1], pixel.0[2]);
                        count += 1.0;
                    }
                }

                let avg = if count == 0.0 { 0.0 } else { total / count };
                buckets[row * FRAME_SIGNATURE_GRID + col] = avg as u8;
            }
        }

        Self { buckets }
    }

    pub fn is_meaningfully_different(&self, previous: &Self) -> bool {
        let mut total = 0_u32;
        let mut peak = 0_u32;
        let mut changed_cells = 0_u32;

        for (current, old) in self.buckets.iter().zip(previous.buckets.iter()) {
            let delta = u32::from((i16::from(*current) - i16::from(*old)).unsigned_abs());
            total += delta;
            peak = peak.max(delta);
            if delta >= FRAME_SIGNATURE_CHANGED_CELL_DELTA {
                changed_cells += 1;
            }
        }

        total >= FRAME_SIGNATURE_TOTAL_DELTA
            || peak >= FRAME_SIGNATURE_PEAK_DELTA
            || changed_cells >= FRAME_SIGNATURE_CHANGED_CELLS
    }
}

pub fn virtual_desktop_bounds() -> Result<DesktopBounds> {
    let screens = Screen::all().context("failed to enumerate displays")?;
    let left = screens
        .iter()
        .map(physical_display_bounds)
        .map(|bounds| bounds.x)
        .min()
        .unwrap_or_default();
    let top = screens
        .iter()
        .map(physical_display_bounds)
        .map(|bounds| bounds.y)
        .min()
        .unwrap_or_default();
    let right = screens
        .iter()
        .map(physical_display_bounds)
        .map(|bounds| bounds.x + bounds.width as i32)
        .max()
        .unwrap_or(1280);
    let bottom = screens
        .iter()
        .map(physical_display_bounds)
        .map(|bounds| bounds.y + bounds.height as i32)
        .max()
        .unwrap_or(720);

    Ok(DesktopBounds {
        x: left,
        y: top,
        width: (right - left).max(1) as u32,
        height: (bottom - top).max(1) as u32,
    })
}

pub fn capture_region(selection: &SelectionRect) -> Result<CapturedFrame> {
    let screens = Screen::all().context("failed to enumerate displays")?;
    let center_x = selection.x + (selection.width as i32 / 2);
    let center_y = selection.y + (selection.height as i32 / 2);
    let screen = screens
        .into_iter()
        .find(|screen| {
            let bounds = physical_display_bounds(screen);
            center_x >= bounds.x
                && center_x < bounds.x + bounds.width as i32
                && center_y >= bounds.y
                && center_y < bounds.y + bounds.height as i32
        })
        .context("selected region is outside the available displays")?;

    let display_bounds = physical_display_bounds(&screen);
    if !selection_fits_bounds(selection, &display_bounds) {
        return Err(anyhow!(
            "selected region must stay within a single display when DPI scaling is enabled"
        ));
    }

    let local_physical_x = selection.x - display_bounds.x;
    let local_physical_y = selection.y - display_bounds.y;
    let local_x = logical_offset(local_physical_x, screen.display_info.scale_factor);
    let local_y = logical_offset(local_physical_y, screen.display_info.scale_factor);
    let capture_width = logical_size(selection.width, screen.display_info.scale_factor);
    let capture_height = logical_size(selection.height, screen.display_info.scale_factor);
    let captured = screen
        .capture_area(local_x, local_y, capture_width, capture_height)
        .map_err(|error| anyhow!(error.to_string()))?;

    let captured_width = captured.width();
    let captured_height = captured.height();
    let image = RgbaImage::from_raw(captured_width, captured_height, captured.into_raw())
        .context("failed to materialize the captured frame")?;

    Ok(CapturedFrame {
        image,
        metadata: CaptureMetadata {
            coordinate_space: CaptureCoordinateSpace::SelectionPhysicalPixels,
            display_origin_x: display_bounds.x,
            display_origin_y: display_bounds.y,
            display_width: display_bounds.width,
            display_height: display_bounds.height,
            capture_origin_x: local_physical_x,
            capture_origin_y: local_physical_y,
            capture_width: captured_width,
            capture_height: captured_height,
            scale_factor: screen.display_info.scale_factor,
        },
    })
}

pub fn estimate_colors(image: &RgbaImage, rect: &PixelRect) -> (String, String) {
    let x0 = rect.x.min(image.width().saturating_sub(1));
    let y0 = rect.y.min(image.height().saturating_sub(1));
    let x1 = (rect.x + rect.width).min(image.width());
    let y1 = (rect.y + rect.height).min(image.height());

    let pad = 6_u32;
    let outer_x0 = x0.saturating_sub(pad);
    let outer_y0 = y0.saturating_sub(pad);
    let outer_x1 = (x1 + pad).min(image.width());
    let outer_y1 = (y1 + pad).min(image.height());

    let mut inner = Vec::new();
    let mut ring = Vec::new();

    for y in outer_y0..outer_y1 {
        for x in outer_x0..outer_x1 {
            let pixel = image.get_pixel(x, y).0;
            if x >= x0 && x < x1 && y >= y0 && y < y1 {
                inner.push(pixel);
            } else {
                ring.push(pixel);
            }
        }
    }

    let background = dominant_color(if ring.is_empty() { &inner } else { &ring });
    let mut scored = inner
        .iter()
        .map(|pixel| (color_distance(*pixel, background), *pixel))
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    let take = (scored.len() / 3).max(1);
    let foreground_candidates = scored
        .iter()
        .rev()
        .take(take)
        .filter(|(distance, _)| *distance >= 18.0)
        .map(|(_, pixel)| *pixel)
        .collect::<Vec<_>>();
    let foreground = dominant_color(if foreground_candidates.is_empty() {
        &inner
    } else {
        &foreground_candidates
    });

    let resolved_foreground = if color_distance(foreground, background) < 28.0 {
        if luminance(background[0], background[1], background[2]) > 148.0 {
            [18, 26, 33, 255]
        } else {
            [244, 239, 229, 255]
        }
    } else {
        foreground
    };

    (to_hex(resolved_foreground), to_hex(background))
}

fn dominant_color(pixels: &[[u8; 4]]) -> [u8; 4] {
    if pixels.is_empty() {
        return [32, 32, 32, 255];
    }

    let mut buckets = HashMap::<(u8, u8, u8), Vec<[u8; 4]>>::new();
    for pixel in pixels {
        let key = (pixel[0] / 16, pixel[1] / 16, pixel[2] / 16);
        buckets.entry(key).or_default().push(*pixel);
    }

    buckets
        .into_values()
        .max_by_key(|bucket| bucket.len())
        .map(|bucket| average_color(&bucket))
        .unwrap_or([32, 32, 32, 255])
}

fn average_color(pixels: &[[u8; 4]]) -> [u8; 4] {
    if pixels.is_empty() {
        return [32, 32, 32, 255];
    }

    let mut red = 0_f32;
    let mut green = 0_f32;
    let mut blue = 0_f32;
    let mut alpha = 0_f32;
    for pixel in pixels {
        red += f32::from(pixel[0]);
        green += f32::from(pixel[1]);
        blue += f32::from(pixel[2]);
        alpha += f32::from(pixel[3]);
    }
    let count = pixels.len() as f32;
    [
        (red / count) as u8,
        (green / count) as u8,
        (blue / count) as u8,
        (alpha / count) as u8,
    ]
}

fn color_distance(a: [u8; 4], b: [u8; 4]) -> f32 {
    let dr = f32::from(a[0]) - f32::from(b[0]);
    let dg = f32::from(a[1]) - f32::from(b[1]);
    let db = f32::from(a[2]) - f32::from(b[2]);
    (dr * dr + dg * dg + db * db).sqrt()
}

fn luminance(red: u8, green: u8, blue: u8) -> f32 {
    0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue)
}

fn to_hex(pixel: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2])
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    #[test]
    fn estimate_colors_prefers_dominant_background_over_ring_noise() {
        let mut image = RgbaImage::from_pixel(48, 24, Rgba([30, 30, 30, 255]));
        for x in 0..6 {
            image.put_pixel(x, 0, Rgba([220, 40, 40, 255]));
        }
        for y in 6..14 {
            for x in 14..26 {
                image.put_pixel(x, y, Rgba([240, 230, 210, 255]));
            }
        }

        let (foreground, background) = estimate_colors(
            &image,
            &PixelRect {
                x: 10,
                y: 4,
                width: 24,
                height: 14,
            },
        );

        assert_eq!(background, "#1E1E1E");
        assert_eq!(foreground, "#F0E6D2");
    }

    #[test]
    fn estimate_colors_falls_back_to_high_contrast_when_inner_block_is_flat() {
        let image = RgbaImage::from_pixel(24, 16, Rgba([236, 234, 228, 255]));

        let (foreground, background) = estimate_colors(
            &image,
            &PixelRect {
                x: 4,
                y: 4,
                width: 14,
                height: 8,
            },
        );

        assert_eq!(background, "#ECEAE4");
        assert_eq!(foreground, "#121A21");
    }
}

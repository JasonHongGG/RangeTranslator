use anyhow::{anyhow, Context, Result};
use image::RgbaImage;
use screenshots::Screen;

use crate::models::{PixelRect, SelectionRect};

const FRAME_SIGNATURE_GRID: usize = 12;
const FRAME_SIGNATURE_BUCKETS: usize = FRAME_SIGNATURE_GRID * FRAME_SIGNATURE_GRID;
const FRAME_SIGNATURE_CHANGED_CELL_DELTA: u32 = 8;
const FRAME_SIGNATURE_PEAK_DELTA: u32 = 16;
const FRAME_SIGNATURE_TOTAL_DELTA: u32 = 120;
const FRAME_SIGNATURE_CHANGED_CELLS: u32 = 4;

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub image: RgbaImage,
}

impl CapturedFrame {
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }
}

#[derive(Debug, Clone)]
pub struct DesktopBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
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
        .map(|screen| screen.display_info.x)
        .min()
        .unwrap_or_default();
    let top = screens
        .iter()
        .map(|screen| screen.display_info.y)
        .min()
        .unwrap_or_default();
    let right = screens
        .iter()
        .map(|screen| screen.display_info.x + screen.display_info.width as i32)
        .max()
        .unwrap_or(1280);
    let bottom = screens
        .iter()
        .map(|screen| screen.display_info.y + screen.display_info.height as i32)
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
            let info = &screen.display_info;
            center_x >= info.x
                && center_x < info.x + info.width as i32
                && center_y >= info.y
                && center_y < info.y + info.height as i32
        })
        .context("selected region is outside the available displays")?;

    let local_x = selection.x - screen.display_info.x;
    let local_y = selection.y - screen.display_info.y;
    let captured = screen
        .capture_area(local_x, local_y, selection.width, selection.height)
        .map_err(|error| anyhow!(error.to_string()))?;

    let image = RgbaImage::from_raw(
        captured.width(),
        captured.height(),
        captured.into_raw(),
    )
    .context("failed to materialize the captured frame")?;

    Ok(CapturedFrame { image })
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

    let background = average_color(if ring.is_empty() { &inner } else { &ring });
    let mut scored = inner
        .iter()
        .map(|pixel| (color_distance(*pixel, background), *pixel))
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    let take = (scored.len() / 5).max(1);
    let foreground = average_color(
        &scored
            .iter()
            .rev()
            .take(take)
            .map(|(_, pixel)| *pixel)
            .collect::<Vec<_>>(),
    );

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
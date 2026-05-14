use anyhow::{anyhow, Context, Result};

use super::{OcrProvider, OcrResult, OcrTextLine};
use crate::capture::CapturedFrame;
use crate::models::PixelRect;

#[cfg(windows)]
pub struct WindowsOcrProvider;

#[cfg(not(windows))]
pub struct UnsupportedOcrProvider;

#[cfg(windows)]
impl OcrProvider for WindowsOcrProvider {
    fn id(&self) -> &'static str {
        "windows-native"
    }

    fn label(&self) -> &'static str {
        "Windows OCR"
    }

    fn recognize(
        &self,
        frame: &CapturedFrame,
        requested_source: &str,
        hint: Option<&str>,
    ) -> Result<OcrResult> {
        recognize_capture(frame, requested_source, hint)
    }
}

#[cfg(not(windows))]
impl OcrProvider for UnsupportedOcrProvider {
    fn id(&self) -> &'static str {
        "unsupported"
    }

    fn label(&self) -> &'static str {
        "Unavailable OCR"
    }

    fn recognize(
        &self,
        frame: &CapturedFrame,
        requested_source: &str,
        hint: Option<&str>,
    ) -> Result<OcrResult> {
        recognize_capture(frame, requested_source, hint)
    }

    fn descriptor(&self) -> ProviderDescriptor {
        crate::models::ProviderDescriptor {
            id: self.id().to_string(),
            label: self.label().to_string(),
            kind: "ocr".to_string(),
            available: false,
            detail: Some("Windows OCR is only available on Windows hosts".to_string()),
        }
    }
}

#[cfg(windows)]
fn recognize_capture(
    frame: &CapturedFrame,
    requested_source: &str,
    hint: Option<&str>,
) -> Result<OcrResult> {
    use std::collections::HashSet;

    use windows::{
        core::HSTRING,
        Globalization::Language,
        Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Storage::Streams::DataWriter,
    };

    fn bitmap_from_frame(frame: &CapturedFrame) -> Result<SoftwareBitmap> {
        let mut bgra = frame.image.clone().into_raw();
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        let writer = DataWriter::new().context("failed to allocate WinRT data writer")?;
        writer.WriteBytes(&bgra).context("failed to copy pixels into WinRT buffer")?;
        let buffer = writer.DetachBuffer().context("failed to detach WinRT buffer")?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            frame.width() as i32,
            frame.height() as i32,
        )
        .context("failed to create a SoftwareBitmap for OCR")?;
        Ok(bitmap)
    }

    fn available_languages() -> Result<HashSet<String>> {
        let mut tags = HashSet::new();
        for language in OcrEngine::AvailableRecognizerLanguages()
            .context("failed to query available OCR languages")?
        {
            tags.insert(
                language
                    .LanguageTag()
                    .context("failed to read OCR language tag")?
                    .to_string(),
            );
        }
        Ok(tags)
    }

    fn score(lines: &[OcrTextLine]) -> usize {
        lines
            .iter()
            .map(|line| line.text.chars().filter(|ch| !ch.is_whitespace()).count())
            .sum()
    }

    fn recognize_with_language(bitmap: &SoftwareBitmap, tag: &str) -> Result<Vec<OcrTextLine>> {
        let language = Language::CreateLanguage(&HSTRING::from(tag))
            .context("failed to create WinRT OCR language")?;
        let engine = OcrEngine::TryCreateFromLanguage(&language)
            .context("failed to initialize Windows OCR engine")?;
        let result = engine
            .RecognizeAsync(bitmap)
            .context("failed to invoke Windows OCR")?
            .get()
            .context("failed while awaiting Windows OCR")?;

        let mut lines = Vec::new();
        for line in result.Lines().context("failed to enumerate OCR lines")? {
            let text = line.Text().context("failed to read OCR line text")?.to_string();
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }

            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = 0_f32;
            let mut max_y = 0_f32;
            for word in line.Words().context("failed to enumerate OCR words")? {
                let rect = word.BoundingRect().context("failed to read OCR word bounds")?;
                min_x = min_x.min(rect.X);
                min_y = min_y.min(rect.Y);
                max_x = max_x.max(rect.X + rect.Width);
                max_y = max_y.max(rect.Y + rect.Height);
            }

            if !min_x.is_finite() || !min_y.is_finite() {
                continue;
            }

            lines.push(OcrTextLine {
                text,
                rect: PixelRect {
                    x: min_x.max(0.0).round() as u32,
                    y: min_y.max(0.0).round() as u32,
                    width: (max_x - min_x).max(1.0).round() as u32,
                    height: (max_y - min_y).max(1.0).round() as u32,
                },
                confidence: 0.86,
            });
        }

        Ok(lines)
    }

    let available = available_languages()?;
    let common_candidates = [
        hint.unwrap_or_default(),
        "en-US",
        "ja-JP",
        "ko-KR",
        "zh-Hans",
        "zh-TW",
        "fr-FR",
        "de-DE",
        "es-ES",
        "ru-RU",
        "th-TH",
        "vi-VN",
        "id-ID",
    ];

    let mut candidates = Vec::new();
    if requested_source != "auto" {
        candidates.push(requested_source.to_string());
    }
    for candidate in common_candidates {
        if !candidate.is_empty()
            && !candidates.iter().any(|tag| tag == candidate)
            && available.contains(candidate)
        {
            candidates.push(candidate.to_string());
        }
    }

    if candidates.is_empty() {
        candidates.extend(available.into_iter());
    }

    let bitmap = bitmap_from_frame(frame)?;
    let mut best: Option<(String, Vec<OcrTextLine>, usize)> = None;
    for candidate in candidates {
        let lines = recognize_with_language(&bitmap, &candidate)
            .with_context(|| format!("Windows OCR failed for {candidate}"))?;
        let current_score = score(&lines);
        let should_replace = best
            .as_ref()
            .map(|(_, _, best_score)| current_score > *best_score)
            .unwrap_or(true);

        if should_replace {
            best = Some((candidate, lines, current_score));
        }
    }

    let (language, lines, _) =
        best.ok_or_else(|| anyhow!("Windows OCR returned no usable result"))?;
    Ok(OcrResult { language, lines })
}

#[cfg(not(windows))]
fn recognize_capture(
    _frame: &CapturedFrame,
    _requested_source: &str,
    _hint: Option<&str>,
) -> Result<OcrResult> {
    Err(anyhow!("Windows OCR is only available on Windows hosts"))
}

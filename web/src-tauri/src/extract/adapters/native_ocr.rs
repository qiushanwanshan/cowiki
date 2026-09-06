//! OS-native OCR adapter for raster images.
//!
//! macOS uses the Vision framework and Windows uses Windows.Media.Ocr. Both
//! are built into the OS, work offline, and need no user-installed
//! dependency — unlike the previous Tesseract/PaddleOCR sidecars, which
//! forced non-technical users to install system tools by hand.
//!
//! Platforms without a system OCR provider get a clear error.

use std::path::Path;
use std::time::Instant;

use crate::extract::contract::{
    ExtractResult, ExtractStats, ExtractionMode, Provenance, SourceFormat,
};

#[cfg(target_os = "macos")]
mod imp {
    use std::path::Path;

    use objc2::AllocAnyThread;
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };

    pub const EXTRACTOR: &str = "apple-vision";
    pub const VERSION: &str = "vision-1";
    pub const SETTINGS_HASH: &str = "zh-hans+en-us/accurate";

    pub fn recognize_text(path: &Path) -> Result<String, String> {
        let ns_path = NSString::from_str(&path.to_string_lossy());
        let url = NSURL::fileURLWithPath(&ns_path);
        let options = NSDictionary::new();
        let handler = unsafe {
            VNImageRequestHandler::initWithURL_options(
                VNImageRequestHandler::alloc(),
                &url,
                &options,
            )
        };

        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setRecognitionLanguages(&NSArray::from_slice(&[
            &*NSString::from_str("zh-Hans"),
            &*NSString::from_str("en-US"),
        ]));
        request.setUsesLanguageCorrection(true);

        let request_ref: &VNRequest = &request;
        handler
            .performRequests_error(&NSArray::from_slice(&[request_ref]))
            .map_err(|error| format!("Vision OCR failed: {}", error.localizedDescription()))?;

        let Some(observations) = request.results() else {
            return Ok(String::new());
        };
        let mut lines = Vec::new();
        for observation in observations.iter() {
            let candidates = observation.topCandidates(1);
            if let Some(candidate) = candidates.firstObject() {
                let text = candidate.string().to_string();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
        }
        Ok(lines.join("\n"))
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::path::Path;

    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
        ExifOrientationMode,
    };
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::{FileAccessMode, StorageFile};
    use windows_future::Async;

    pub const EXTRACTOR: &str = "windows-media-ocr";
    pub const VERSION: &str = "winocr-1";
    pub const SETTINGS_HASH: &str = "zh-hans+profile";

    pub fn recognize_text(path: &Path) -> Result<String, String> {
        // StorageFile requires an absolute path.
        let path = path
            .canonicalize()
            .map_err(|error| format!("cannot resolve image path: {error}"))?;
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))
            .map_err(|error| format!("cannot open image: {error}"))?
            .join()
            .map_err(|error| format!("cannot open image: {error}"))?;
        let stream = file
            .OpenAsync(FileAccessMode::Read)
            .map_err(|error| format!("cannot read image: {error}"))?
            .join()
            .map_err(|error| format!("cannot read image: {error}"))?;
        let decoder = BitmapDecoder::CreateAsync(&stream)
            .and_then(|operation| operation.join())
            .map_err(|error| format!("cannot decode image: {error}"))?;

        // Windows.Media.Ocr rejects images larger than MaxImageDimension on
        // either axis, so downscale oversized screenshots first.
        let max_dimension = OcrEngine::MaxImageDimension()
            .map_err(|error| format!("cannot query OCR limits: {error}"))?;
        let width = decoder
            .PixelWidth()
            .map_err(|error| format!("cannot read image size: {error}"))?;
        let height = decoder
            .PixelHeight()
            .map_err(|error| format!("cannot read image size: {error}"))?;
        let transform = BitmapTransform::new()
            .map_err(|error| format!("cannot prepare image transform: {error}"))?;
        if width > max_dimension || height > max_dimension {
            let scale = max_dimension as f64 / width.max(height) as f64;
            transform
                .SetScaledWidth(((width as f64 * scale).floor() as u32).max(1))
                .map_err(|error| format!("cannot scale image: {error}"))?;
            transform
                .SetScaledHeight(((height as f64 * scale).floor() as u32).max(1))
                .map_err(|error| format!("cannot scale image: {error}"))?;
        }
        let bitmap = decoder
            .GetSoftwareBitmapTransformedAsync(
                BitmapPixelFormat::Gray8,
                BitmapAlphaMode::Ignore,
                &transform,
                ExifOrientationMode::IgnoreExifOrientation,
                ColorManagementMode::DoNotColorManage,
            )
            .and_then(|operation| operation.join())
            .map_err(|error| format!("cannot rasterize image: {error}"))?;

        let chinese = Language::CreateLanguage(&HSTRING::from("zh-Hans"))
            .map_err(|error| format!("cannot create OCR language: {error}"))?;
        let engine = OcrEngine::TryCreateFromLanguage(&chinese)
            .or_else(|_| OcrEngine::TryCreateFromUserProfileLanguages())
            .map_err(|_| {
                "no OCR language available; install the Chinese (Simplified) language pack in Windows Settings > Time & Language".to_string()
            })?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .and_then(|operation| operation.join())
            .map_err(|error| format!("Windows OCR failed: {error}"))?;
        Ok(result
            .Text()
            .map_err(|error| format!("Windows OCR failed: {error}"))?
            .to_string_lossy())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use std::path::Path;

    pub const EXTRACTOR: &str = "native-ocr";
    pub const VERSION: &str = "none";
    pub const SETTINGS_HASH: &str = "unsupported";

    pub fn recognize_text(_path: &Path) -> Result<String, String> {
        Err("image OCR is not available on this platform yet (supported: macOS, Windows)"
            .to_string())
    }
}

pub fn extract_image_result(path: &Path) -> Result<ExtractResult, String> {
    let started = Instant::now();
    let text = imp::recognize_text(path)?.trim().to_string();
    if text.is_empty() {
        return Err("OCR produced no text".to_string());
    }
    let mut provenance = Provenance::local_fast(
        imp::EXTRACTOR,
        imp::VERSION,
        started.elapsed().as_millis() as u64,
    );
    provenance.mode = ExtractionMode::LocalOcr;
    provenance.settings_hash = imp::SETTINGS_HASH.to_string();
    Ok(ExtractResult::from_adapter(
        text.clone(),
        SourceFormat::Image,
        ExtractStats::from_text(&text),
        provenance,
    ))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::path::Path;

    /// Manual smoke test against a real image:
    /// `COWIKI_OCR_TEST_IMAGE=/path/to.png cargo test vision_ocr -- --nocapture`
    #[test]
    fn vision_ocr_extracts_text_from_a_real_image() {
        let Some(image) = std::env::var_os("COWIKI_OCR_TEST_IMAGE") else {
            return;
        };
        let text = super::imp::recognize_text(Path::new(&image)).unwrap();
        assert!(!text.trim().is_empty(), "no text recognized");
        println!("{text}");
    }
}

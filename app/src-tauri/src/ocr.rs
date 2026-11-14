//! Reads the text in a picture with Windows' built-in text recognition (Windows.Media.Ocr).
//! It runs entirely on this computer, with the language packs Windows already has; nothing is sent
//! anywhere. Used only when the person turns on "Read text in pictures" for a folder scan.

use std::path::Path;

#[cfg(windows)]
pub fn read_image_text(path: &Path) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
        ExifOrientationMode,
    };
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::{FileAccessMode, StorageFile};

    thread_local! {
        static INIT: () = unsafe {
            // Each scan thread joins the multithreaded apartment once; "already joined" is fine.
            let _ = windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
        };
    }
    INIT.with(|_| {});

    let run = || -> windows::core::Result<String> {
        let full = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(full.as_os_str()))?.get()?;
        let stream = file.OpenAsync(FileAccessMode::Read)?.get()?;
        let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
        let (w, h) = (decoder.OrientedPixelWidth()?, decoder.OrientedPixelHeight()?);
        if w < 64 || h < 32 {
            return Ok(String::new()); // icons and the like hold no readable text
        }
        // The engine has a maximum picture size; scale bigger pictures down to fit.
        let max = OcrEngine::MaxImageDimension()?.max(1);
        let scale = (max as f64 / w.max(h) as f64).min(1.0);
        let transform = BitmapTransform::new()?;
        transform.SetScaledWidth(((w as f64 * scale) as u32).max(1))?;
        transform.SetScaledHeight(((h as f64 * scale) as u32).max(1))?;
        transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
        let bitmap = decoder
            .GetSoftwareBitmapTransformedAsync(
                BitmapPixelFormat::Bgra8,
                BitmapAlphaMode::Premultiplied,
                &transform,
                ExifOrientationMode::RespectExifOrientation,
                ColorManagementMode::DoNotColorManage,
            )?
            .get()?;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let result = engine.RecognizeAsync(&bitmap)?.get()?;
        let mut out = String::new();
        for line in result.Lines()? {
            out.push_str(&line.Text()?.to_string_lossy());
            out.push('\n');
        }
        Ok(out)
    };
    run().ok().filter(|t| !t.trim().is_empty())
}

#[cfg(not(windows))]
pub fn read_image_text(_path: &Path) -> Option<String> {
    None
}

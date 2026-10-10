use crate::ingest::IngestionError;
use std::path::{Path, PathBuf};

pub struct AppleVisionOcr;

impl Default for AppleVisionOcr {
    fn default() -> Self {
        Self::new()
    }
}

impl AppleVisionOcr {
    pub fn new() -> Self {
        Self
    }

    pub fn can_handle(&self, extension: &str) -> bool {
        matches!(
            extension.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "tiff" | "tif" | "webp" | "bmp"
        )
    }

    #[cfg(target_os = "macos")]
    pub fn get_or_compile_helper() -> Option<PathBuf> {
        let bin_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".serchx")
            .join("bin");
        let binary_path = bin_dir.join("serchx-vision-ocr-v2");

        if binary_path.exists() {
            return Some(binary_path);
        }

        let _ = std::fs::create_dir_all(&bin_dir);
        let swift_src = bin_dir.join("vision_ocr_v2.swift");
        let swift_code = r#"
import Foundation
import Vision
import CoreGraphics
import ImageIO

let args = CommandLine.arguments
guard args.count > 1 else { exit(1) }
let path = args[1]
let url = URL(fileURLWithPath: path)

guard let src = CGImageSourceCreateWithURL(url as CFURL, nil) else { exit(0) }

let maxDim = 2560
let thumbOpts: [CFString: Any] = [
    kCGImageSourceCreateThumbnailFromImageAlways: true,
    kCGImageSourceCreateThumbnailWithTransform: true,
    kCGImageSourceThumbnailMaxPixelSize: maxDim
]

guard let cgImage = CGImageSourceCreateThumbnailAtIndex(src, 0, thumbOpts as CFDictionary)
    ?? CGImageSourceCreateImageAtIndex(src, 0, nil) else { exit(0) }

if cgImage.width < 64 && cgImage.height < 64 {
    exit(0)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .fast
request.usesLanguageCorrection = false

let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
do {
    try handler.perform([request])
    guard let observations = request.results else { exit(0) }
    let recognizedStrings = observations.compactMap { $0.topCandidates(1).first?.string }
    print(recognizedStrings.joined(separator: "\n"))
} catch {
    fputs("Error: \(error)", stderr)
    exit(1)
}
"#;

        if std::fs::write(&swift_src, swift_code).is_err() {
            return None;
        }

        let status = std::process::Command::new("/usr/bin/swiftc")
            .arg("-O")
            .arg(&swift_src)
            .arg("-o")
            .arg(&binary_path)
            .status();

        if let Ok(st) = status {
            if st.success() && binary_path.exists() {
                return Some(binary_path);
            }
        }

        None
    }

    #[cfg(target_os = "macos")]
    pub fn recognize_text(&self, image_path: &Path) -> Result<String, IngestionError> {
        use std::process::Command;

        if !image_path.exists() {
            return Err(IngestionError::Ocr(format!(
                "Image file not found: {:?}",
                image_path
            )));
        }

        // Fast-path: skip files smaller than 2KB (icons, favicons, bullets have no readable text)
        if let Ok(meta) = std::fs::metadata(image_path) {
            if meta.len() < 2048 {
                return Ok(String::new());
            }
        }

        // Serialize OCR execution to prevent subprocess stampedes across worker threads
        static OCR_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = OCR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let helper_opt = Self::get_or_compile_helper();
        let mut cmd = if let Some(ref helper) = helper_opt {
            let mut c = Command::new(helper);
            c.arg(image_path);
            c
        } else {
            let swift_code = r#"
import Foundation
import Vision
import CoreGraphics
import ImageIO

let args = CommandLine.arguments
guard args.count > 1 else { exit(1) }
let path = args[1]
let url = URL(fileURLWithPath: path)

guard let src = CGImageSourceCreateWithURL(url as CFURL, nil) else { exit(0) }

let maxDim = 2560
let thumbOpts: [CFString: Any] = [
    kCGImageSourceCreateThumbnailFromImageAlways: true,
    kCGImageSourceCreateThumbnailWithTransform: true,
    kCGImageSourceThumbnailMaxPixelSize: maxDim
]

guard let cgImage = CGImageSourceCreateThumbnailAtIndex(src, 0, thumbOpts as CFDictionary)
    ?? CGImageSourceCreateImageAtIndex(src, 0, nil) else { exit(0) }

if cgImage.width < 64 && cgImage.height < 64 {
    exit(0)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .fast
request.usesLanguageCorrection = false

let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
do {
    try handler.perform([request])
    guard let observations = request.results else { exit(0) }
    let recognizedStrings = observations.compactMap { $0.topCandidates(1).first?.string }
    print(recognizedStrings.joined(separator: "\n"))
} catch {
    fputs("Error: \(error)", stderr)
    exit(1)
}
"#;
            let mut c = Command::new("/usr/bin/swift");
            c.arg("-e").arg(swift_code).arg(image_path);
            c
        };

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| IngestionError::Ocr(format!("Failed to spawn Vision OCR: {}", e)))?;

        let pid = child.id();
        let timeout = std::time::Duration::from_secs(15);
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(50);

        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        tracing::warn!("Vision OCR timed out for {:?}, killing child process {}", image_path, pid);
                        let _ = child.kill();
                        unsafe { libc::kill(pid as i32, libc::SIGKILL); }
                        let _ = child.wait();
                        return Err(IngestionError::Ocr(format!(
                            "Vision OCR timed out after {} seconds",
                            timeout.as_secs()
                        )));
                    }
                    std::thread::sleep(poll_interval);
                }
                Err(e) => {
                    let _ = child.kill();
                    unsafe { libc::kill(pid as i32, libc::SIGKILL); }
                    let _ = child.wait();
                    return Err(IngestionError::Ocr(format!("Vision OCR execution failed: {}", e)));
                }
            }
        };

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        if let Some(mut out) = child.stdout.take() {
            use std::io::Read;
            let _ = out.read_to_end(&mut stdout);
        }
        if let Some(mut err) = child.stderr.take() {
            use std::io::Read;
            let _ = err.read_to_end(&mut stderr);
        }

        if !status.success() {
            let err = String::from_utf8_lossy(&stderr);
            return Err(IngestionError::Ocr(format!("Vision OCR error: {}", err)));
        }

        Ok(String::from_utf8_lossy(&stdout).trim().to_string())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn recognize_text(&self, _image_path: &Path) -> Result<String, IngestionError> {
        // Fallback for non-macOS platforms
        Ok("Mock OCR output for non-macOS test runner".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_vision_runner() {
        let ocr = AppleVisionOcr::new();
        assert!(ocr.can_handle("png"));
        assert!(ocr.can_handle("JPG"));
        assert!(ocr.can_handle("webp"));
        assert!(!ocr.can_handle("pdf"));
        assert!(!ocr.can_handle("txt"));

        // Non-existent image returns clean error
        let non_existent = Path::new("/tmp/non_existent_image_12345.png");
        let res = ocr.recognize_text(non_existent);
        assert!(res.is_err());
    }
}

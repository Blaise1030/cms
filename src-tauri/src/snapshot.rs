//! Review screenshots in the desktop app: a native snapshot of an area of the app's own webview.
//!
//! WKWebView's `takeSnapshot(with:completionHandler:)` draws the webview's content (the review's
//! preview frames included) without the Screen Recording permission a screen capture needs.

use base64::Engine;

/// An area of the window, in its CSS pixels (WKWebView points at page zoom 1).
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Region {
    /// Inside the window and at least a pixel each way, or an error saying why not.
    pub fn checked(self, window_width: f64, window_height: f64) -> Result<Region, String> {
        let finite = [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite());
        if !finite || self.width < 1.0 || self.height < 1.0 {
            return Err("Pick an area to capture".into());
        }
        let x = self.x.clamp(0.0, window_width);
        let y = self.y.clamp(0.0, window_height);
        let width = self.width.min(window_width - x);
        let height = self.height.min(window_height - y);
        if width < 1.0 || height < 1.0 {
            return Err("The area is outside the window".into());
        }
        Ok(Region {
            x,
            y,
            width,
            height,
        })
    }
}

/// `data:image/png;base64,…` for PNG bytes.
pub fn png_data_url(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

/// The PNG of `region` of `window`'s webview.
#[cfg(target_os = "macos")]
pub async fn capture(window: &tauri::WebviewWindow, region: Region) -> Result<String, String> {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSImage;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::NSError;
    use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};

    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let region = region.checked(size.width as f64 / scale, size.height as f64 / scale)?;
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<Vec<u8>, String>>();
    let tx = std::sync::Mutex::new(Some(tx));
    window
        .with_webview(move |webview| {
            // `with_webview` runs this on the main thread.
            let Some(mtm) = MainThreadMarker::new() else {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(Err("The snapshot must run on the main thread".into()));
                }
                return;
            };
            // SAFETY: on macOS `inner()` is the WKWebView.
            let view: &WKWebView = unsafe { &*webview.inner().cast() };
            let config = unsafe { WKSnapshotConfiguration::new(mtm) };
            let rect = CGRect::new(
                CGPoint::new(region.x, region.y),
                CGSize::new(region.width, region.height),
            );
            unsafe { config.setRect(rect) };
            let done = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
                let result = if image.is_null() {
                    let why = unsafe { error.as_ref() }
                        .map(|e| e.localizedDescription().to_string())
                        .unwrap_or_else(|| "The snapshot failed".into());
                    Err(why)
                } else {
                    let image: &NSImage = unsafe { &*image };
                    png_of(image)
                };
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(result);
                }
            });
            unsafe { view.takeSnapshotWithConfiguration_completionHandler(Some(&config), &done) };
        })
        .map_err(|e| e.to_string())?;
    let png = rx
        .await
        .map_err(|_| "The snapshot was dropped".to_string())??;
    Ok(png_data_url(&png))
}

/// An `NSImage` as PNG bytes.
#[cfg(target_os = "macos")]
fn png_of(image: &objc2_app_kit::NSImage) -> Result<Vec<u8>, String> {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
    let tiff = image
        .TIFFRepresentation()
        .ok_or("The snapshot had no pixels")?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff).ok_or("The snapshot couldn't be read")?;
    let props = objc2_foundation::NSDictionary::new();
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) }
        .ok_or("The snapshot couldn't be encoded")?;
    Ok(png.to_vec())
}

#[cfg(not(target_os = "macos"))]
pub async fn capture(_window: &tauri::WebviewWindow, _region: Region) -> Result<String, String> {
    Err("Screenshots from the desktop app need macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_are_kept_inside_the_window() {
        let r = Region {
            x: 1300.0,
            y: -5.0,
            width: 400.0,
            height: 100.0,
        }
        .checked(1440.0, 900.0)
        .unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (1300.0, 0.0, 140.0, 100.0));
        assert!(Region {
            x: 0.0,
            y: 0.0,
            width: 0.5,
            height: 10.0
        }
        .checked(1440.0, 900.0)
        .is_err());
        assert!(Region {
            x: 2000.0,
            y: 0.0,
            width: 10.0,
            height: 10.0
        }
        .checked(1440.0, 900.0)
        .is_err());
        assert!(Region {
            x: f64::NAN,
            y: 0.0,
            width: 10.0,
            height: 10.0
        }
        .checked(1440.0, 900.0)
        .is_err());
    }

    #[test]
    fn pngs_become_data_urls() {
        assert_eq!(png_data_url(&[0x89, b'P']), "data:image/png;base64,iVA=");
    }
}

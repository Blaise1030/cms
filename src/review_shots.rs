//! Screenshot comments on a change: what a reviewer queued (a note per picked area, most with
//! a screenshot), checked, stored on the repository's `orso-review-assets` branch, and written
//! into one pull request review.
//!
//! Images live in the same repository so they are visible exactly to those who can see the
//! change; the branch is an orphan that holds only them and is never merged.

use base64::Engine;
use serde::Deserialize;

use crate::error::ApiError;

/// The branch the screenshots are committed to.
pub const ASSETS_BRANCH: &str = "orso-review-assets";
/// Most items in one review.
pub const MAX_ITEMS: usize = 10;
/// Largest screenshot, decoded.
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// The request body limit: every image at its largest, base64-encoded, plus the notes.
pub const MAX_BODY_BYTES: usize = MAX_ITEMS * (MAX_IMAGE_BYTES * 4 / 3 + 4096) + 64 * 1024;

/// GitHub's review event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewEvent {
    Comment,
    Approve,
    RequestChanges,
}

impl ReviewEvent {
    pub fn as_github(self) -> &'static str {
        match self {
            Self::Comment => "COMMENT",
            Self::Approve => "APPROVE",
            Self::RequestChanges => "REQUEST_CHANGES",
        }
    }
}

/// Which frame the area was picked on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Change,
    Live,
}

/// One queued item as the web sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotItemIn {
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub path: String,
    pub side: Side,
    #[serde(default)]
    pub viewport: String,
    /// Absent for a note added without a screenshot.
    #[serde(default)]
    pub image_base64: Option<String>,
    #[serde(default)]
    pub mime: Option<String>,
}

/// Body of `POST /api/projects/{id}/reviews/{n}/review-with-shots`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReviewIn {
    pub event: ReviewEvent,
    #[serde(default)]
    pub body: String,
    pub head_sha: String,
    #[serde(default)]
    pub items: Vec<ShotItemIn>,
}

/// A checked item: its image decoded (with the file extension it gets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShotItem {
    pub note: String,
    pub path: String,
    pub side: Side,
    pub viewport: String,
    pub image: Option<(Vec<u8>, &'static str)>,
}

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];

/// Screenshot types, as the picker makes them.
const SHOT_TYPES: &[&str] = &["image/png", "image/jpeg"];
/// Images attached to a comment.
const UPLOAD_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];
/// Most images on one comment.
pub const MAX_UPLOADS: usize = 10;
/// Largest attached image, decoded.
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
/// The comment request body limit: every image at its largest, base64-encoded, plus the text.
pub const MAX_COMMENT_BODY_BYTES: usize =
    MAX_UPLOADS * (MAX_UPLOAD_BYTES * 4 / 3 + 1024) + 256 * 1024;

/// An image attached to a comment, as the web sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct UploadIn {
    pub base64: String,
    pub mime: String,
    #[serde(default)]
    pub name: String,
}

/// The file extension for an image type.
fn extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        _ => "webp",
    }
}

/// "PNG, JPEG, GIF or WebP" for a list of image types.
fn type_names(allowed: &[&str]) -> String {
    let n: Vec<_> = allowed
        .iter()
        .map(|m| match *m {
            "image/png" => "PNG",
            "image/jpeg" => "JPEG",
            "image/gif" => "GIF",
            _ => "WebP",
        })
        .collect();
    match n.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => n.join(""),
    }
}

/// `b64` decoded, checked to be one of `allowed` by its own bytes (not just the declared
/// type) and at most `max` bytes, with its file extension. `label` names it in errors.
pub fn decode_image(
    b64: &str,
    mime: Option<&str>,
    allowed: &[&str],
    max: usize,
    label: &str,
) -> Result<(Vec<u8>, &'static str), ApiError> {
    let Some(mime) = mime.filter(|m| allowed.contains(m)) else {
        return Err(ApiError::badreq(format!(
            "{label} must be a {} image",
            type_names(allowed)
        )));
    };
    let too_big = || ApiError::badreq(format!("{label} is larger than {} MB", max / (1024 * 1024)));
    // Over the limit before decoding is over the limit after it.
    if b64.len() > max * 4 / 3 + 4 {
        return Err(too_big());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| ApiError::badreq(format!("{label} isn't valid base64")))?;
    if bytes.len() > max {
        return Err(too_big());
    }
    if image_mime(&bytes) != Some(mime) {
        return Err(ApiError::badreq(format!(
            "{label} isn't the image type it says it is"
        )));
    }
    Ok((bytes, extension(mime)))
}

/// Checks a comment's images: at most 10, PNG, JPEG, GIF or WebP (by their bytes), 5 MB each.
pub fn validate_uploads(images: &[UploadIn]) -> Result<Vec<(Vec<u8>, &'static str)>, ApiError> {
    if images.len() > MAX_UPLOADS {
        return Err(ApiError::badreq(format!(
            "Attach at most {MAX_UPLOADS} images to a comment"
        )));
    }
    images
        .iter()
        .enumerate()
        .map(|(i, img)| {
            decode_image(
                &img.base64,
                Some(&img.mime),
                UPLOAD_TYPES,
                MAX_UPLOAD_BYTES,
                &format!("Image {}", i + 1),
            )
        })
        .collect()
}

/// A comment's text with its images appended, one `![image](url)` per line.
pub fn comment_markdown(body: &str, image_urls: &[String]) -> String {
    let mut out = body.trim().to_string();
    for url in image_urls {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&format!("![image]({url})"));
    }
    out
}

/// Checks the request: at most 10 items, PNG or JPEG only (by their bytes, not just the
/// declared type), 2 MB each, a summary when sending back, and something to say at all.
pub fn validate(req: &ShotReviewIn) -> Result<Vec<ShotItem>, ApiError> {
    if req.items.len() > MAX_ITEMS {
        return Err(ApiError::badreq(format!(
            "Send at most {MAX_ITEMS} screenshot comments at a time"
        )));
    }
    if req.event == ReviewEvent::RequestChanges && req.body.trim().is_empty() {
        return Err(ApiError::badreq(
            "Say what needs changing before sending it back",
        ));
    }
    if req.event == ReviewEvent::Comment && req.body.trim().is_empty() && req.items.is_empty() {
        return Err(ApiError::badreq("Nothing to send"));
    }
    req.items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let n = i + 1;
            let image = match &item.image_base64 {
                None => None,
                Some(b64) => Some(decode_image(
                    b64,
                    item.mime.as_deref(),
                    SHOT_TYPES,
                    MAX_IMAGE_BYTES,
                    &format!("Screenshot {n}"),
                )?),
            };
            if image.is_none() && item.note.trim().is_empty() {
                return Err(ApiError::badreq(format!(
                    "Comment {n} has neither a note nor a screenshot"
                )));
            }
            Ok(ShotItem {
                note: item.note.trim().to_string(),
                path: clean_path(&item.path),
                side: item.side,
                viewport: one_line(&item.viewport),
                image,
            })
        })
        .collect()
}

/// A page path for display: one line, starting with `/`, without backticks.
fn clean_path(path: &str) -> String {
    let p = one_line(path).replace('`', "");
    if p.starts_with('/') {
        p
    } else {
        format!("/{p}")
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A file orso put on the assets branch: `shots/` or `uploads/`, then `{pr}/{date}/{uuid}.{ext}`
/// with an image extension. Anything else (and anything with `..`) is refused.
pub fn is_asset_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let [kind, pr, date, file] = parts.as_slice() else {
        return false;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let date_ok = date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    let Some((stem, ext)) = file.rsplit_once('.') else {
        return false;
    };
    matches!(*kind, "shots" | "uploads")
        && digits(pr)
        && date_ok
        && stem.len() == 36
        && stem.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        && matches!(ext, "png" | "jpg" | "gif" | "webp")
}

/// A commit sha (7 to 40 hex digits).
pub fn is_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The image type of `bytes` by their first bytes; None for anything else.
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(PNG_MAGIC) {
        Some("image/png")
    } else if bytes.starts_with(JPEG_MAGIC) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// `{folder}/{pr}/{yyyy-mm-dd}/{uuid}.{ext}` for an image saved `unix_secs` after the epoch;
/// `folder` is `shots` (screenshot comments) or `uploads` (images attached to comments).
pub fn asset_path(folder: &str, pr: u64, unix_secs: u64, id: &str, ext: &str) -> String {
    let (y, m, d) = civil_date(unix_secs / 86_400);
    format!("{folder}/{pr}/{y:04}-{m:02}-{d:02}/{id}.{ext}")
}

/// A random v4 UUID.
pub fn new_id() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian (Howard Hinnant's algorithm).
fn civil_date(days: u64) -> (i64, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The review body: the summary, then each item as a numbered section with its note, where it
/// was taken, and its screenshot (`image_urls[i]`, when it has one).
pub fn review_markdown(summary: &str, items: &[ShotItem], image_urls: &[Option<String>]) -> String {
    let mut out = summary.trim().to_string();
    for (i, item) in items.iter().enumerate() {
        if !out.is_empty() {
            // One blank line between sections.
            out.truncate(out.trim_end().len());
            out.push_str("\n\n");
        }
        let title = item.note.lines().next().unwrap_or("").trim();
        let title = if title.is_empty() {
            "Screenshot"
        } else {
            title
        };
        out.push_str(&format!("### {}. {title}\n", i + 1));
        if !item.note.is_empty() {
            out.push_str(&format!("\n{}\n", item.note));
        }
        let side = match item.side {
            Side::Change => "change",
            Side::Live => "live",
        };
        let place = if item.viewport.is_empty() {
            side.to_string()
        } else {
            format!("{side}, {}", item.viewport)
        };
        out.push_str(&format!("\nPage: `{}` ({place})\n", item.path));
        if let Some(Some(url)) = image_urls.get(i) {
            out.push_str(&format!("\n![screenshot]({url})\n"));
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        let mut b = PNG_MAGIC.to_vec();
        b.extend_from_slice(b"rest");
        b
    }
    fn b64(b: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(b)
    }
    fn item(image: Option<(&[u8], &str)>, note: &str) -> ShotItemIn {
        ShotItemIn {
            note: note.into(),
            path: "/pricing".into(),
            side: Side::Change,
            viewport: "desktop".into(),
            image_base64: image.map(|(b, _)| b64(b)),
            mime: image.map(|(_, m)| m.to_string()),
        }
    }
    fn req(event: ReviewEvent, body: &str, items: Vec<ShotItemIn>) -> ShotReviewIn {
        ShotReviewIn {
            event,
            body: body.into(),
            head_sha: "h".into(),
            items,
        }
    }

    #[test]
    fn accepts_png_jpeg_and_notes_without_images() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2];
        let items = validate(&req(
            ReviewEvent::Comment,
            "",
            vec![
                item(Some((&png(), "image/png")), "Hero is cut off"),
                item(Some((&jpeg, "image/jpeg")), ""),
                item(None, "No shot, just a note"),
            ],
        ))
        .unwrap();
        assert_eq!(items[0].image.as_ref().unwrap().1, "png");
        assert_eq!(items[1].image.as_ref().unwrap().1, "jpg");
        assert!(items[2].image.is_none());
    }

    #[test]
    fn refuses_bad_requests() {
        let bad = |r: ShotReviewIn| validate(&r).unwrap_err().message;
        assert!(bad(req(ReviewEvent::Comment, "", vec![])).contains("Nothing to send"));
        assert!(
            bad(req(ReviewEvent::RequestChanges, " ", vec![item(None, "x")]))
                .contains("needs changing")
        );
        let many = (0..11).map(|_| item(None, "x")).collect();
        assert!(bad(req(ReviewEvent::Comment, "", many)).contains("at most 10"));
        // Declared PNG, but the bytes are a GIF.
        assert!(bad(req(
            ReviewEvent::Comment,
            "",
            vec![item(Some((b"GIF89a....", "image/png")), "x")]
        ))
        .contains("isn't the image type"));
        assert!(bad(req(
            ReviewEvent::Comment,
            "",
            vec![item(Some((&png(), "image/gif")), "x")]
        ))
        .contains("PNG or JPEG"));
        let mut huge = png();
        huge.resize(MAX_IMAGE_BYTES + 1, 0);
        assert!(bad(req(
            ReviewEvent::Comment,
            "",
            vec![item(Some((&huge, "image/png")), "x")]
        ))
        .contains("larger than 2 MB"));
        assert!(bad(req(ReviewEvent::Comment, "", vec![item(None, "  ")])).contains("neither"));
    }

    #[test]
    fn asset_paths_are_dated_and_unique() {
        // 2026-11-19T12:00:00Z
        assert_eq!(
            asset_path("shots", 83, 1_795_089_600, "abc", "png"),
            "shots/83/2026-11-19/abc.png"
        );
        assert_eq!(
            asset_path("uploads", 1, 0, "x", "jpg"),
            "uploads/1/1970-01-01/x.jpg"
        );
        let (a, b) = (new_id(), new_id());
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
    }

    #[test]
    fn markdown_numbers_each_item_with_its_page_and_image() {
        let items = vec![
            ShotItem {
                note: "Hero is cut off\nOn mobile too".into(),
                path: "/pricing".into(),
                side: Side::Change,
                viewport: "mobile 390px".into(),
                image: Some((png(), "png")),
            },
            ShotItem {
                note: "Old copy here".into(),
                path: "/".into(),
                side: Side::Live,
                viewport: "desktop".into(),
                image: None,
            },
        ];
        let md = review_markdown(
            "Two things.",
            &items,
            &[
                Some("https://github.com/o/r/blob/c1/shots/a.png?raw=true".into()),
                None,
            ],
        );
        assert_eq!(
            md,
            "Two things.\n\n### 1. Hero is cut off\n\nHero is cut off\nOn mobile too\n\n\
             Page: `/pricing` (change, mobile 390px)\n\n\
             ![screenshot](https://github.com/o/r/blob/c1/shots/a.png?raw=true)\n\n\
             ### 2. Old copy here\n\nOld copy here\n\nPage: `/` (live, desktop)"
        );
    }

    #[test]
    fn only_orso_asset_paths_and_shas_are_served() {
        let id = "0f8fad5b-d9cb-469f-a165-70867728950e";
        assert!(is_asset_path(&format!("shots/83/2026-11-19/{id}.png")));
        assert!(is_asset_path(&format!("uploads/7/2026-01-02/{id}.webp")));
        for bad in [
            format!("site/83/2026-11-19/{id}.png"),
            format!("shots/../2026-11-19/{id}.png"),
            format!("shots/83/2026-11-19/{id}.svg"),
            format!("shots/83/26-11-19/{id}.png"),
            "shots/83/2026-11-19/x.png".to_string(),
            format!("shots/83/2026-11-19/{id}.png/extra"),
            ".github/workflows/x.yml".to_string(),
        ] {
            assert!(!is_asset_path(&bad), "{bad}");
        }
        assert!(is_sha("abc1234"));
        assert!(!is_sha("main"));
        assert!(!is_sha("abc"));
    }

    #[test]
    fn image_types_come_from_the_bytes() {
        assert_eq!(image_mime(&png()), Some("image/png"));
        assert_eq!(image_mime(&[0xFF, 0xD8, 0xFF, 0]), Some("image/jpeg"));
        assert_eq!(image_mime(b"GIF89a.."), Some("image/gif"));
        assert_eq!(image_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_mime(b"<svg onload=alert(1)>"), None);
    }

    #[test]
    fn comment_images_allow_gif_and_webp_up_to_5_mb_and_ten() {
        let up = |b: &[u8], m: &str| UploadIn {
            base64: b64(b),
            mime: m.into(),
            name: "x".into(),
        };
        let ok = validate_uploads(&[
            up(&png(), "image/png"),
            up(b"GIF89a....", "image/gif"),
            up(b"RIFF\0\0\0\0WEBPVP8 ", "image/webp"),
        ])
        .unwrap();
        assert_eq!(
            ok.iter().map(|(_, e)| *e).collect::<Vec<_>>(),
            ["png", "gif", "webp"]
        );
        let err = |r: Result<Vec<(Vec<u8>, &str)>, ApiError>| r.unwrap_err().message;
        assert!(err(validate_uploads(&[up(b"<svg/>", "image/svg+xml")]))
            .contains("PNG, JPEG, GIF or WebP"));
        assert!(
            err(validate_uploads(&[up(b"GIF89a", "image/png")])).contains("isn't the image type")
        );
        let mut big = png();
        big.resize(MAX_UPLOAD_BYTES + 1, 0);
        assert!(err(validate_uploads(&[up(&big, "image/png")])).contains("larger than 5 MB"));
        let many: Vec<_> = (0..11).map(|_| up(&png(), "image/png")).collect();
        assert!(err(validate_uploads(&many)).contains("at most 10"));
    }

    #[test]
    fn comment_markdown_appends_each_image() {
        let urls = vec!["https://u/1".to_string(), "https://u/2".to_string()];
        assert_eq!(
            comment_markdown(" Look ", &urls),
            "Look\n\n![image](https://u/1)\n\n![image](https://u/2)"
        );
        assert_eq!(comment_markdown("", &urls[..1]), "![image](https://u/1)");
    }

    #[test]
    fn paths_are_one_line_and_rooted() {
        assert_eq!(clean_path("pricing"), "/pricing");
        assert_eq!(clean_path("/a`b\n/c"), "/ab /c");
    }
}

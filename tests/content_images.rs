//! Task 9: transactional image save through multipart create/update.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::config::Config;
use workbench::content::images::{self, ImageFormat, MAX_IMAGE_BYTES};
use workbench::state::AppState;
use workbench::{changes, http};

const WELCOME: &str = "src/content/blog/en/welcome.md";
const PNG: &[u8] = include_bytes!("fixtures/images/pixel.png");
const JPEG: &[u8] = include_bytes!("fixtures/images/pixel.jpg");
const WEBP: &[u8] = include_bytes!("fixtures/images/pixel.webp");
const WEBP_LOSSLESS: &[u8] = include_bytes!("fixtures/images/pixel-lossless.webp");
const GIF: &[u8] = include_bytes!("fixtures/images/pixel.gif");
const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3"><script>alert(1)</script></svg>"#;

fn git_cli(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn link_node_modules(dir: &Path) {
    let modules =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/astro-marketing/node_modules");
    std::os::unix::fs::symlink(modules, dir.join("node_modules")).unwrap();
    let exclude = dir.join(".git/info/exclude");
    let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
    text.push_str("\n/node_modules\n");
    std::fs::write(exclude, text).unwrap();
}

struct Env {
    tmp: tempfile::TempDir,
    app: axum::Router,
    id: String,
    dir: PathBuf,
}

async fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(
        Config {
            port: 0,
            origin: None,
            projects_dir: tmp.path().to_path_buf(),
            no_auth: true,
            token: None,
            agents_config: None,
            ..Default::default()
        },
        false,
    );
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    link_node_modules(&dir);
    changes::create(&dir, "Add images").await.unwrap();
    Env {
        app: http::router(state.clone()),
        tmp,
        id: info.id,
        dir,
    }
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn read(e: &Env, collection: &str, id: &str) -> Value {
    let req = Request::builder()
        .uri(format!(
            "/api/content/entry?project={}&collection={collection}&id={id}",
            e.id
        ))
        .header(header::HOST, "localhost")
        .body(Body::empty())
        .unwrap();
    let (s, v) = send(&e.app, req).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v
}

struct Part<'a> {
    client_id: &'a str,
    filename: &'a str,
    /// What the browser claims; the daemon must ignore it.
    mime: &'a str,
    bytes: &'a [u8],
}

const BOUNDARY: &str = "workbench-test-boundary-7MA4YWxk";

fn multipart(entry: &Value, parts: &[Part]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"entry\"\r\nContent-Type: application/json\r\n\r\n").bytes());
    out.extend(entry.to_string().bytes());
    out.extend(b"\r\n");
    for p in parts {
        out.extend(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
                p.client_id, p.filename, p.mime
            )
            .bytes(),
        );
        out.extend(p.bytes);
        out.extend(b"\r\n");
    }
    out.extend(format!("--{BOUNDARY}--\r\n").bytes());
    out
}

async fn save(e: &Env, method: &str, entry: Value, parts: &[Part<'_>]) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri("/api/content/entry")
        .header(header::HOST, "localhost")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(multipart(&entry, parts)))
        .unwrap();
    send(&e.app, req).await
}

/// An update of `welcome` carrying image metadata.
fn update(e: &Env, opened: &Value, fields: Value, body: Option<&str>, images: Value) -> Value {
    let mut v = json!({
        "project": e.id, "collection": "blog", "id": "welcome",
        "branch": opened["branch"], "hash": opened["hash"], "fields": fields, "images": images,
    });
    if let Some(body) = body {
        v["body"] = json!(body);
    }
    v
}

fn image(client_id: &str, alt: &str, filename: &str) -> Value {
    json!({"clientId": client_id, "alt": alt, "filename": filename})
}

fn png_part<'a>(client_id: &'a str, filename: &'a str, bytes: &'a [u8]) -> Part<'a> {
    Part {
        client_id,
        filename,
        mime: "image/png",
        bytes,
    }
}

fn head(dir: &Path) -> String {
    git_cli(dir, &["rev-parse", "HEAD"])
}

fn staging_is_empty(dir: &Path) -> bool {
    let staging = dir.join(".git/workbench-staging");
    !staging.exists() || std::fs::read_dir(staging).unwrap().next().is_none()
}

/// A structurally valid PNG padded with a private ancillary chunk to exactly `len` bytes.
fn padded_png(len: usize) -> Vec<u8> {
    let iend = PNG.len() - 12;
    let pad = len - PNG.len() - 12;
    let mut out = PNG[..iend].to_vec();
    out.extend((pad as u32).to_be_bytes());
    out.extend(b"zzZz");
    out.resize(out.len() + pad, 0);
    out.extend([0, 0, 0, 0]); // CRC of private chunks is not inspected
    out.extend(&PNG[iend..]);
    assert_eq!(out.len(), len);
    out
}

/// Everything the failed save must have left as it was.
struct Snapshot {
    entry: Vec<u8>,
    head: String,
}

fn snapshot(e: &Env) -> Snapshot {
    Snapshot {
        entry: std::fs::read(e.dir.join(WELCOME)).unwrap(),
        head: head(&e.dir),
    }
}

fn assert_untouched(e: &Env, before: &Snapshot) {
    assert_eq!(std::fs::read(e.dir.join(WELCOME)).unwrap(), before.entry);
    assert_eq!(head(&e.dir), before.head);
    assert!(
        !e.dir.join("src/assets/blog/welcome").exists(),
        "no asset may remain after a failed save"
    );
    assert!(staging_is_empty(&e.dir), "staging must be cleaned up");
}

#[test]
fn byte_inspection_accepts_real_images_up_to_ten_mib() {
    assert_eq!(images::validate_upload(PNG).unwrap(), ImageFormat::Png);
    assert_eq!(images::validate_upload(JPEG).unwrap(), ImageFormat::Jpeg);
    assert_eq!(images::validate_upload(WEBP).unwrap(), ImageFormat::Webp);
    assert_eq!(
        images::validate_upload(WEBP_LOSSLESS).unwrap(),
        ImageFormat::Webp
    );
    assert_eq!(images::validate_upload(GIF).unwrap(), ImageFormat::Gif);
    assert_eq!(MAX_IMAGE_BYTES, 10 * 1024 * 1024);
    let max = padded_png(MAX_IMAGE_BYTES);
    assert_eq!(images::validate_upload(&max).unwrap(), ImageFormat::Png);
}

#[test]
fn byte_inspection_rejects_svg_arbitrary_truncated_and_oversized_bytes() {
    let reason = |bytes: &[u8]| images::validate_upload(bytes).unwrap_err().reason;
    assert_eq!(reason(SVG), Some("unsupported_image"));
    assert_eq!(
        reason(b"hello, I am not an image"),
        Some("unsupported_image")
    );
    assert_eq!(reason(b""), Some("unsupported_image"));
    // Right signature, broken structure.
    assert_eq!(reason(&PNG[..20]), Some("unsupported_image"));
    assert_eq!(reason(&JPEG[..JPEG.len() - 40]), Some("unsupported_image"));
    assert_eq!(reason(&GIF[..GIF.len() - 1]), Some("unsupported_image"));
    assert_eq!(reason(&WEBP[..30]), Some("unsupported_image"));
    let mut corrupt_ihdr = PNG.to_vec();
    corrupt_ihdr[20] ^= 0xff; // width byte; IHDR CRC no longer matches
    assert_eq!(reason(&corrupt_ihdr), Some("unsupported_image"));
    assert_eq!(
        reason(&padded_png(MAX_IMAGE_BYTES + 1)),
        Some("image_too_large")
    );
}

#[test]
fn asset_names_are_sanitized_and_use_the_detected_extension() {
    assert_eq!(
        images::asset_name("My Photo (1).JPEG", ImageFormat::Png),
        "my-photo-1.png"
    );
    assert_eq!(
        images::asset_name("../../etc/passwd", ImageFormat::Gif),
        "passwd.gif"
    );
    assert_eq!(images::asset_name("", ImageFormat::Webp), "image.webp");
    assert_eq!(
        images::asset_name("日本.png", ImageFormat::Jpeg),
        "image.jpg"
    );
}

#[tokio::test]
async fn markdown_image_saves_entry_and_asset_in_one_scoped_checkpoint() {
    let e = env().await;
    // An unrelated edit must not be committed with the content mutation.
    std::fs::write(e.dir.join("README.md"), "unrelated\n").unwrap();
    let opened = read(&e, "blog", "welcome").await;
    let body = format!(
        "{}\n![A \\[red\\] pixel](workbench-image:img-1)\n",
        opened["body"].as_str().unwrap()
    );
    let entry = update(
        &e,
        &opened,
        json!({}),
        Some(&body),
        json!([image("img-1", "A [red] pixel", "My Photo.png")]),
    );
    let (s, v) = save(&e, "PUT", entry, &[png_part("img-1", "My Photo.png", PNG)]).await;
    assert_eq!(s, StatusCode::OK, "{v}");

    let asset = "src/assets/blog/welcome/my-photo.png";
    assert_eq!(std::fs::read(e.dir.join(asset)).unwrap(), PNG);
    assert_eq!(
        v["images"]["img-1"],
        json!({"path": asset, "link": "../../../assets/blog/welcome/my-photo.png"})
    );
    let link = "![A \\[red\\] pixel](../../../assets/blog/welcome/my-photo.png)";
    assert!(v["body"].as_str().unwrap().contains(link), "{}", v["body"]);
    let source = std::fs::read_to_string(e.dir.join(WELCOME)).unwrap();
    assert!(source.contains(link));
    assert!(!source.contains("workbench-image:"));

    let committed = git_cli(&e.dir, &["show", "--format=", "--name-only", "HEAD"]);
    let mut committed: Vec<_> = committed.lines().collect();
    committed.sort();
    assert_eq!(committed, vec![asset, WELCOME]);
    assert!(git_cli(&e.dir, &["status", "--porcelain"]).contains("README.md"));
    assert!(staging_is_empty(&e.dir));
}

#[tokio::test]
async fn frontmatter_image_field_gets_relative_path_with_alt_companion() {
    let e = env().await;
    let opened = read(&e, "blog", "welcome").await;
    let entry = update(
        &e,
        &opened,
        json!({"coverImage": "workbench-image:cover", "coverAlt": "Team photo"}),
        None,
        json!([image("cover", "Team photo", "cover.jpg")]),
    );
    let (s, v) = save(
        &e,
        "PUT",
        entry,
        &[Part {
            client_id: "cover",
            filename: "cover.jpg",
            mime: "image/jpeg",
            bytes: JPEG,
        }],
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["fields"]["coverImage"],
        "../../../assets/blog/welcome/cover.jpg"
    );
    assert_eq!(v["fields"]["coverAlt"], "Team photo");
    assert_eq!(
        std::fs::read(e.dir.join("src/assets/blog/welcome/cover.jpg")).unwrap(),
        JPEG
    );
}

#[tokio::test]
async fn collisions_get_deterministic_unique_names_and_extension_follows_bytes() {
    let e = env().await;
    let assets = e.dir.join("src/assets/blog/welcome");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("photo.png"), b"existing").unwrap();
    let opened = read(&e, "blog", "welcome").await;
    let body =
        "![One](workbench-image:a)\n![Two](workbench-image:b)\n![Three](workbench-image:c)\n";
    let entry = update(
        &e,
        &opened,
        json!({}),
        Some(body),
        json!([
            image("a", "One", "Photo.PNG"),
            image("b", "Two", "photo.png"),
            image("c", "Three", "photo.gif")
        ]),
    );
    let (s, v) = save(
        &e,
        "PUT",
        entry,
        &[
            png_part("a", "Photo.PNG", PNG),
            png_part("b", "photo.png", PNG),
            // Named .gif, labelled PNG, actually a WebP.
            png_part("c", "photo.gif", WEBP),
        ],
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        std::fs::read(assets.join("photo.png")).unwrap(),
        b"existing"
    );
    assert_eq!(std::fs::read(assets.join("photo-2.png")).unwrap(), PNG);
    assert_eq!(std::fs::read(assets.join("photo-3.png")).unwrap(), PNG);
    assert_eq!(std::fs::read(assets.join("photo.webp")).unwrap(), WEBP);
    let body = v["body"].as_str().unwrap();
    assert!(body.contains("![One](../../../assets/blog/welcome/photo-2.png)"));
    assert!(body.contains("![Two](../../../assets/blog/welcome/photo-3.png)"));
    assert!(body.contains("![Three](../../../assets/blog/welcome/photo.webp)"));
}

#[tokio::test]
async fn create_with_image_places_it_under_the_new_slug() {
    let e = env().await;
    let entry = json!({
        "project": e.id, "collection": "blog", "slug": "launch", "branch": read(&e, "blog", "welcome").await["branch"],
        "fields": {"title":"Launch","description":"New","pubDate":"2026-10-12","coverImage":"workbench-image:hero","coverAlt":"Hero"},
        "body": "Hello\n\n![Chart](workbench-image:chart)\n",
        "images": [image("hero", "Hero", "hero.gif"), image("chart", "Chart", "chart.png")],
    });
    // A body larger than Axum's 2 MB default must fit.
    let big = padded_png(3 * 1024 * 1024);
    let (s, v) = save(
        &e,
        "POST",
        entry,
        &[
            Part {
                client_id: "hero",
                filename: "hero.gif",
                mime: "image/gif",
                bytes: GIF,
            },
            png_part("chart", "chart.png", &big),
        ],
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["fields"]["coverImage"],
        "../../../assets/blog/launch/hero.gif"
    );
    assert!(v["body"]
        .as_str()
        .unwrap()
        .contains("![Chart](../../../assets/blog/launch/chart.png)"));
    assert_eq!(
        std::fs::read(e.dir.join("src/assets/blog/launch/chart.png")).unwrap(),
        big
    );
    let committed = git_cli(&e.dir, &["show", "--format=", "--name-only", "HEAD"]);
    assert_eq!(committed.lines().count(), 3, "{committed}");
}

#[tokio::test]
async fn mislabeled_svg_or_arbitrary_bytes_roll_back_everything() {
    let e = env().await;
    let before = snapshot(&e);
    let opened = read(&e, "blog", "welcome").await;
    for bytes in [SVG, b"GIF89a but not really".as_slice(), &PNG[..30]] {
        let body = "![Good](workbench-image:good)\n![Bad](workbench-image:bad)\n";
        let entry = update(
            &e,
            &opened,
            json!({}),
            Some(body),
            json!([
                image("good", "Good", "good.png"),
                image("bad", "Bad", "bad.png")
            ]),
        );
        let (s, v) = save(
            &e,
            "PUT",
            entry,
            &[
                png_part("good", "good.png", PNG),
                png_part("bad", "bad.png", bytes),
            ],
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        assert_eq!(v["error"]["reason"], "unsupported_image", "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains("bad.png"),
            "{v}"
        );
        assert_untouched(&e, &before);
    }
}

#[tokio::test]
async fn oversized_image_is_rejected_with_a_clear_reason() {
    let e = env().await;
    let before = snapshot(&e);
    let opened = read(&e, "blog", "welcome").await;
    let big = padded_png(MAX_IMAGE_BYTES + 1);
    let entry = update(
        &e,
        &opened,
        json!({}),
        Some("![Big](workbench-image:big)\n"),
        json!([image("big", "Big", "big.png")]),
    );
    let (s, v) = save(&e, "PUT", entry, &[png_part("big", "big.png", &big)]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"]["reason"], "image_too_large", "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("10 MiB"),
        "{v}"
    );
    assert_untouched(&e, &before);
}

#[tokio::test]
async fn blank_alt_unknown_and_unused_images_are_rejected() {
    let e = env().await;
    let before = snapshot(&e);
    let opened = read(&e, "blog", "welcome").await;
    let cases = [
        // Blank alt text.
        (
            Some("![x](workbench-image:a)\n"),
            json!([image("a", "  ", "a.png")]),
            "missing_alt",
        ),
        // The body names an image that was not uploaded.
        (
            Some("![x](workbench-image:zzz)\n"),
            json!([image("a", "A", "a.png")]),
            "invalid_image",
        ),
        // An uploaded image the entry never uses would become an orphan asset.
        (
            Some("No image\n"),
            json!([image("a", "A", "a.png")]),
            "invalid_image",
        ),
        // Metadata without a part.
        (
            Some("![x](workbench-image:a)\n![y](workbench-image:b)\n"),
            json!([image("a", "A", "a.png"), image("b", "B", "b.png")]),
            "invalid_image",
        ),
    ];
    for (body, images, reason) in cases {
        let entry = update(&e, &opened, json!({}), body, images);
        let (s, v) = save(&e, "PUT", entry, &[png_part("a", "a.png", PNG)]).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        assert_eq!(v["error"]["reason"], reason, "{v}");
        assert_untouched(&e, &before);
    }
}

#[tokio::test]
async fn schema_validation_failure_rolls_back_staged_assets() {
    let e = env().await;
    let before = snapshot(&e);
    let opened = read(&e, "blog", "welcome").await;
    // coverImage without coverAlt fails the blog schema's refinement.
    let entry = update(
        &e,
        &opened,
        json!({"coverImage": "workbench-image:cover"}),
        None,
        json!([image("cover", "Cover", "cover.png")]),
    );
    let (s, v) = save(&e, "PUT", entry, &[png_part("cover", "cover.png", PNG)]).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_schema", "{v}");
    assert_untouched(&e, &before);
    assert!(!e.dir.join("src/assets").exists());
}

#[tokio::test]
async fn io_failure_after_placing_assets_rolls_them_back() {
    let e = env().await;
    let before = snapshot(&e);
    let opened = read(&e, "blog", "welcome").await;
    let entry_dir = e.dir.join("src/content/blog/en");
    // The entry's temporary file cannot be created; the assets are already placed.
    std::fs::set_permissions(&entry_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let entry = update(
        &e,
        &opened,
        json!({}),
        Some("![A](workbench-image:a)\n"),
        json!([image("a", "A", "a.png")]),
    );
    let (s, v) = save(&e, "PUT", entry, &[png_part("a", "a.png", PNG)]).await;
    std::fs::set_permissions(&entry_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert_untouched(&e, &before);
    assert!(
        !e.dir.join("src/assets").exists(),
        "created directories are removed too"
    );
}

async fn assert_symlink_refused(e: &Env, link: &str) {
    let outside = tempfile::tempdir().unwrap();
    let before = snapshot(e);
    let path = e.dir.join(link);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(outside.path(), &path).unwrap();
    let opened = read(e, "blog", "welcome").await;
    let entry = update(
        e,
        &opened,
        json!({}),
        Some("![A](workbench-image:a)\n"),
        json!([image("a", "A", "a.png")]),
    );
    let (s, v) = save(e, "PUT", entry, &[png_part("a", "a.png", PNG)]).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["error"]["reason"], "unsafe_path", "{v}");
    assert_eq!(std::fs::read(e.dir.join(WELCOME)).unwrap(), before.entry);
    assert_eq!(head(&e.dir), before.head);
    assert_eq!(
        std::fs::read_dir(outside.path()).unwrap().count(),
        0,
        "nothing written outside"
    );
    assert!(staging_is_empty(&e.dir));
}

#[tokio::test]
async fn symlinked_collection_asset_directory_is_refused() {
    let e = env().await;
    assert_symlink_refused(&e, "src/assets/blog").await;
}

#[tokio::test]
async fn symlinked_slug_asset_directory_is_refused() {
    let e = env().await;
    assert_symlink_refused(&e, "src/assets/blog/welcome").await;
}

#[tokio::test]
async fn orphan_staging_and_temp_files_are_cleaned_on_the_next_operation() {
    let e = env().await;
    let crashed = e.dir.join(".git/workbench-staging/crashed-op");
    std::fs::create_dir_all(&crashed).unwrap();
    std::fs::write(crashed.join("0"), PNG).unwrap();
    let entry_tmp = e
        .dir
        .join("src/content/blog/en/.welcome.md.99999-0.workbench-tmp");
    std::fs::write(&entry_tmp, "half").unwrap();
    let assets = e.dir.join("src/assets/blog/welcome");
    std::fs::create_dir_all(&assets).unwrap();
    let asset_tmp = assets.join(".photo.png.99999-1.workbench-tmp");
    std::fs::write(&asset_tmp, "half").unwrap();
    // A file the writer owns that merely looks similar stays.
    let keep = assets.join("notes.workbench-tmp.png");
    std::fs::write(&keep, "keep").unwrap();

    // The next content operation is an ordinary JSON save without images.
    let opened = read(&e, "blog", "welcome").await;
    let req = Request::builder()
        .method("PUT")
        .uri("/api/content/entry")
        .header(header::HOST, "localhost")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"project":e.id,"collection":"blog","id":"welcome","branch":opened["branch"],"hash":opened["hash"],"fields":{"title":"Renamed title"}})
                .to_string(),
        ))
        .unwrap();
    let (s, v) = send(&e.app, req).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(v.get("images").is_none(), "JSON saves keep the plain entry shape");
    assert!(staging_is_empty(&e.dir));
    assert!(!entry_tmp.exists());
    assert!(!asset_tmp.exists());
    assert!(keep.exists());
    let _ = &e.tmp;
}

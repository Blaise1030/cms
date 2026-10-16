//! Embedded assets: the scaffold template (`templates/astro-marketing/`) and the built SPA
//! (`web/dist`), which the daemon serves as its router fallback.

use std::io;
use std::path::{Component, Path, PathBuf};

use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::error::ApiError;

/// The built web app. `web/dist` is produced by `pnpm -C web build` (`just build`); a checkout
/// that has not built it still compiles, and the daemon then answers with a build hint.
#[derive(RustEmbed)]
#[folder = "web/dist/"]
#[allow_missing = true]
#[exclude = ".DS_Store"]
pub struct WebApp;

/// Paths owned by the daemon (API, event socket, preview proxy, Vite root modules); never
/// served from the SPA.
pub fn is_daemon_path(path: &str) -> bool {
    ["/api", "/ws", "/preview"].iter().any(|p| {
        path.strip_prefix(p)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }) || crate::preview::is_vite_dev_path(path)
}

/// Client-side route prefixes the SPA owns (deep links fall back to `index.html`).
const SPA_ROUTE_PREFIXES: [&str; 3] = ["/p/", "/assets/", "/icons/"];

/// Whether an unauthenticated request may be answered: an explicit allowlist of the public app
/// shell (static SPA files). The shell holds no secrets and must load without a token (browsers
/// fetch the manifest and service worker without credentials); everything it then talks to is
/// authenticated.
///
/// This fails closed: a path is public only if it is `/`, an SPA route prefix, or a file
/// actually embedded from `web/dist`. A route added to the router later (`/metrics`,
/// `/healthz`, ...) therefore requires a token unless someone deliberately lists it here.
pub fn is_public_shell_request(method: &Method, path: &str) -> bool {
    if !matches!(*method, Method::GET | Method::HEAD) || is_daemon_path(path) {
        return false;
    }
    path == "/"
        || SPA_ROUTE_PREFIXES.iter().any(|p| path.starts_with(p))
        || WebApp::get(path.trim_start_matches('/')).is_some()
}

/// Whether the SPA fallback may answer this request (any GET/HEAD outside the daemon paths).
/// This only decides what `serve` does once a request has passed auth; it is NOT an auth rule.
fn serves_spa(method: &Method, path: &str) -> bool {
    matches!(*method, Method::GET | Method::HEAD) && !is_daemon_path(path)
}

fn cache_control(path: &str) -> &'static str {
    // Vite fingerprints everything under assets/. All else (index.html, sw.js, manifest, ...)
    // must revalidate so app updates and service-worker updates are picked up.
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

fn asset_response(path: &str, headers: &HeaderMap, method: &Method) -> Option<Response> {
    let file = WebApp::get(path)?;
    let etag = format!("\"{}\"", hex(&file.metadata.sha256_hash()));
    let not_modified = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim() == etag));
    let mut res = if not_modified {
        StatusCode::NOT_MODIFIED.into_response()
    } else if *method == Method::HEAD {
        StatusCode::OK.into_response()
    } else {
        file.data.into_owned().into_response()
    };
    let h = res.headers_mut();
    if !not_modified {
        if let Ok(v) = HeaderValue::from_str(file.metadata.mimetype()) {
            h.insert(header::CONTENT_TYPE, v);
        }
    }
    if let Ok(v) = HeaderValue::from_str(&etag) {
        h.insert(header::ETAG, v);
    }
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control(path)),
    );
    Some(res)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Router fallback: serve the SPA. Unknown daemon paths and non-GET methods keep the uniform
/// JSON 404; unknown extension-less paths get `index.html` (client-side routing); unknown
/// files (`/assets/missing.js`) are a real 404 rather than HTML masquerading as a script.
pub async fn serve(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path();
    if !serves_spa(&method, path) {
        return ApiError::enoent("route not found").into_response();
    }
    let rel = path.trim_start_matches('/');
    // Project routes may end in a file name (`/p/site/code/src/index.astro`) yet are still pages.
    let looks_like_file =
        !path.starts_with("/p/") && rel.rsplit('/').next().is_some_and(|seg| seg.contains('.'));
    let target = if rel.is_empty() { "index.html" } else { rel };
    if let Some(res) = asset_response(target, &headers, &method) {
        return res;
    }
    if looks_like_file {
        return ApiError::enoent("route not found").into_response();
    }
    asset_response("index.html", &headers, &method).unwrap_or_else(|| {
        ApiError::enoent("web app not built: run `just build` (or `pnpm -C web build`)")
            .into_response()
    })
}

/// The Astro scaffold copied into every new project. Build artifacts that may exist in a
/// developer checkout are excluded so they never leak into scaffolds.
#[derive(RustEmbed)]
#[folder = "templates/astro-marketing/"]
#[exclude = "node_modules/**"]
#[exclude = "dist/**"]
#[exclude = ".astro/**"]
#[exclude = ".DS_Store"]
pub struct Template;

/// Relative paths of every embedded template file (forward-slash separated).
pub fn template_paths() -> Vec<String> {
    let mut v: Vec<String> = Template::iter().map(|p| p.into_owned()).collect();
    v.sort();
    v
}

/// Write the embedded template into `dest` (created if missing). Existing files are overwritten.
pub fn extract_template(dest: &Path) -> io::Result<()> {
    for rel in Template::iter() {
        let file = Template::get(&rel)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, rel.to_string()))?;
        let target = safe_join(dest, &rel)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, file.data.as_ref())?;
    }
    Ok(())
}

/// Join an embedded relative path onto `base`, refusing anything that could escape it.
fn safe_join(base: &Path, rel: &str) -> io::Result<PathBuf> {
    let mut out = base.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsafe template path: {rel}"),
                ))
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_contains_content_model_and_excludes_build_artifacts() {
        let paths = template_paths();
        for want in [
            "package.json",
            "package-lock.json",
            "AGENTS.md",
            "DESIGN.md",
            "README.md",
            ".gitignore",
            ".github/workflows/deploy.yml",
            "wrangler.toml",
            "astro.config.mjs",
            "src/content.config.ts",
            "src/site.ts",
            "src/content/blog/en/welcome.md",
            "src/pages/index.astro",
        ] {
            assert!(paths.iter().any(|p| p == want), "missing {want}: {paths:?}");
        }
        assert!(
            !paths
                .iter()
                .any(|p| p.starts_with("node_modules/") || p.starts_with("dist/")),
            "{paths:?}"
        );
    }

    #[test]
    fn safe_join_rejects_traversal() {
        assert!(safe_join(Path::new("/x"), "../evil").is_err());
        assert!(safe_join(Path::new("/x"), "/abs").is_err());
        assert!(safe_join(Path::new("/x"), "a/b.txt").is_ok());
    }
}

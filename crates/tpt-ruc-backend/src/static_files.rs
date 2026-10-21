// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Serves the built web app from `TPT_RUC_PWA_DIR`, when the owner sets it.
//!
//! Only files under that folder are served. A request path is refused if it
//! contains an encoded or backslash character, a `..` or hidden segment, or if
//! the resolved file lies outside the folder (for example through a symlink).
//!
//! Paths map the way a Next.js static export expects: `/vehicles` finds
//! `vehicles.html` or `vehicles/index.html`, and `/` finds `index.html`.

use std::path::{Component, Path, PathBuf};

use bytes::Bytes;
use http::{header, HeaderValue, Response, StatusCode};
use http_body_util::Full;
use serde_json::json;

use crate::json_response;

/// Prefix of the content-hashed files a Next.js build writes. Their names change
/// with their content, so browsers may cache them for a long time.
const HASHED_PREFIX: &str = "_next/static/";

/// Answers a GET or HEAD for a page or asset under `root`.
pub async fn serve(root: &Path, request_path: &str, head_only: bool) -> Response<Full<Bytes>> {
    let root = root.to_path_buf();
    let request_path = request_path.to_owned();
    // Looking up and reading a file both touch the disk, so both run on a blocking thread.
    let found = tokio::task::spawn_blocking(move || {
        let file = locate(&root, &request_path)?;
        match std::fs::read(&file.path) {
            Ok(body) => Some((file, body)),
            Err(error) => {
                tracing::warn!(%error, path = %file.path.display(), "could not read a static file");
                None
            }
        }
    })
    .await
    .ok()
    .flatten();

    let Some((file, body)) = found else {
        return json_response(StatusCode::NOT_FOUND, &json!({ "error": "not found" }));
    };

    let mut response = Response::new(Full::new(if head_only {
        Bytes::new()
    } else {
        Bytes::from(body)
    }));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(file.content_type),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if file.hashed {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// A file found under the root, with what the response needs to know about it.
struct Located {
    path: PathBuf,
    content_type: &'static str,
    hashed: bool,
}

/// Finds the file for a request path, or `None` when it should not be served.
fn locate(root: &Path, request_path: &str) -> Option<Located> {
    let relative = clean_relative(request_path)?;
    let root = std::fs::canonicalize(root).ok()?;

    let candidates = if request_path.is_empty() || request_path.ends_with('/') {
        vec![relative.join("index.html")]
    } else {
        vec![
            relative.clone(),
            relative.join("index.html"),
            with_html(&relative),
        ]
    };

    for candidate in candidates {
        let joined = root.join(&candidate);
        let Ok(resolved) = std::fs::canonicalize(&joined) else {
            continue;
        };
        if !resolved.starts_with(&root) || !resolved.is_file() {
            continue;
        }
        let name = candidate.to_string_lossy().replace('\\', "/");
        return Some(Located {
            content_type: content_type_for(&name),
            hashed: name.starts_with(HASHED_PREFIX),
            path: resolved,
        });
    }
    None
}

/// Turns a request path into a relative path of normal segments, or `None`
/// when any part of it is unsafe.
fn clean_relative(request_path: &str) -> Option<PathBuf> {
    if request_path.contains(['%', '\\', '\0']) {
        return None;
    }
    let mut relative = PathBuf::new();
    for segment in request_path.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment.starts_with('.') {
            return None;
        }
        let mut parts = Path::new(segment).components();
        match (parts.next(), parts.next()) {
            (Some(Component::Normal(name)), None) => relative.push(name),
            _ => return None,
        }
    }
    Some(relative)
}

/// `vehicles` → `vehicles.html`, for pages exported without a folder.
fn with_html(relative: &Path) -> PathBuf {
    let mut name = relative.as_os_str().to_owned();
    name.push(".html");
    PathBuf::from(name)
}

/// The media type for a file, chosen by its extension.
fn content_type_for(name: &str) -> &'static str {
    let extension = name.rsplit('.').next().unwrap_or("");
    match extension.to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_paths_are_refused() {
        assert!(clean_relative("/../secret").is_none());
        assert!(clean_relative("/a/../b").is_none());
        assert!(clean_relative("/%2e%2e/secret").is_none());
        assert!(clean_relative("/a\\b").is_none());
        assert!(clean_relative("/.env").is_none());
    }

    #[test]
    fn plain_paths_are_kept() {
        assert_eq!(clean_relative("/").unwrap(), PathBuf::new());
        assert_eq!(
            clean_relative("/_next/static/app.js").unwrap(),
            Path::new("_next").join("static").join("app.js")
        );
    }

    #[test]
    fn extensions_map_to_media_types() {
        assert_eq!(content_type_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type_for("pkg/app_bg.WASM"), "application/wasm");
        assert_eq!(content_type_for("x.bin"), "application/octet-stream");
    }

    #[test]
    fn pages_and_assets_are_found_and_escapes_are_not() {
        let dir = std::env::temp_dir().join(format!("tpt-ruc-static-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("vehicles")).unwrap();
        std::fs::write(dir.join("index.html"), "home").unwrap();
        std::fs::write(dir.join("vehicles").join("index.html"), "vehicles").unwrap();
        std::fs::write(dir.join("about.html"), "about").unwrap();
        std::fs::write(dir.join("secret.txt"), "x").unwrap();

        let home = locate(&dir, "/").unwrap();
        assert!(home.path.ends_with("index.html"));
        assert!(locate(&dir, "/vehicles")
            .unwrap()
            .path
            .ends_with("index.html"));
        assert!(locate(&dir, "/vehicles/")
            .unwrap()
            .path
            .ends_with("index.html"));
        assert!(locate(&dir, "/about").unwrap().path.ends_with("about.html"));
        assert!(locate(&dir, "/missing").is_none());
        assert!(locate(&dir, "/../tpt-ruc-static-x/secret").is_none());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

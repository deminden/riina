//! Shared confined resources for plugin sidebars and overlays.
use std::{
    borrow::Cow,
    fs,
    path::{Component, Path, PathBuf},
};
use url::Url;
use wry::http::{Request, Response, StatusCode};
pub(super) fn plugin_file_url(path: &Path) -> Result<String, String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Sidebar files must use a safe relative plugin path".into());
    }
    let mut url = Url::parse("riina-plugin://localhost/").map_err(|error| error.to_string())?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "Could not build a plugin sidebar URL".to_owned())?;
        segments.pop_if_empty();
        for component in path.components() {
            let Component::Normal(value) = component else {
                return Err("Sidebar files must use a safe relative plugin path".into());
            };
            let value = value
                .to_str()
                .ok_or_else(|| "Sidebar file names must be valid Unicode".to_owned())?;
            segments.push(value);
        }
    }
    Ok(url.into())
}

pub(super) fn resource_response(
    root: &Path,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let body = || {
        let raw_path = request
            .uri()
            .path()
            .strip_prefix('/')
            .ok_or(StatusCode::BAD_REQUEST)?;
        let mut path = PathBuf::new();
        for segment in raw_path.split('/') {
            let decoded = percent_encoding::percent_decode_str(segment)
                .decode_utf8()
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            if decoded.is_empty()
                || Path::new(decoded.as_ref())
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || decoded.contains(['/', '\\', '\0'])
            {
                return Err(StatusCode::FORBIDDEN);
            }
            path.push(decoded.as_ref());
        }
        let file = root
            .join(path)
            .canonicalize()
            .map_err(|_| StatusCode::NOT_FOUND)?;
        if !file.starts_with(root) || !file.is_file() {
            return Err(StatusCode::FORBIDDEN);
        }
        let bytes = fs::read(&file).map_err(|_| StatusCode::NOT_FOUND)?;
        Ok((file, bytes))
    };
    match body() {
        Ok((path, bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type(&path))
            .header(
                "Content-Security-Policy",
                "default-src 'self' data: blob:; img-src 'self' data: blob:; media-src 'self' data: blob:; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self'; frame-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'",
            )
            .body(Cow::Owned(bytes))
            .unwrap_or_else(|_| Response::new(Cow::Owned(Vec::new()))),
        Err(status) => Response::builder()
            .status(status)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Cow::Owned(Vec::new()))
            .unwrap_or_else(|_| Response::new(Cow::Owned(Vec::new()))),
    }
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

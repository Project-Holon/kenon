//! The `kenon` URI scheme: serves the asset bridge to the web view.
//!
//! The front end builds URLs with `convertFileSrc(path, "kenon")`, which
//! percent-encodes the whole path, so the handler decodes it first. Paths
//! are those of `kenon_core::bridge`.

use std::sync::Arc;

use kenon_core::bridge::{Bridge, Error};
use tauri::http::{header, Response, StatusCode};

/// Builds the response for a request path (with its query, if any).
pub fn respond(bridge: Option<Arc<Bridge>>, path: &str, query: Option<&str>) -> Response<Vec<u8>> {
    let text = |status: StatusCode, body: String| {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(body.into_bytes())
            .expect("static response parts")
    };
    let Some(bridge) = bridge else {
        return text(
            StatusCode::SERVICE_UNAVAILABLE,
            "no game data folder is set".to_string(),
        );
    };
    let mut path = percent_decode(path.trim_start_matches('/'));
    if let Some(q) = query.filter(|q| !q.is_empty()) {
        path.push('?');
        path.push_str(&percent_decode(q));
    }
    match bridge.serve(&path) {
        Ok(asset) => {
            let mut response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, asset.mime)
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, "x-kenon-origin")
                .header(header::CACHE_CONTROL, "max-age=3600");
            if let Some((x, y)) = asset.origin {
                response = response.header("x-kenon-origin", format!("{x},{y}"));
            }
            response
                .body(asset.bytes.clone())
                .expect("static response parts")
        }
        Err(err @ Error::BadPath(_)) => text(StatusCode::BAD_REQUEST, err.to_string()),
        Err(err @ Error::Missing(_)) => text(StatusCode::NOT_FOUND, err.to_string()),
        Err(err @ Error::Format(_)) => text(StatusCode::UNPROCESSABLE_ENTITY, err.to_string()),
    }
}

/// Decodes `%XX` escapes as UTF-8. A malformed escape is kept as written.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode() {
        assert_eq!(percent_decode("text%2F23%3A0%2Fa%20b"), "text/23:0/a b");
        assert_eq!(percent_decode("%E3%81%82"), "\u{3042}");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn without_data() {
        let r = respond(None, "/sheet%2F5%3A9", None);
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn bad_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/a")).unwrap();
        let kenon_core::data_root::DataRootCheck::Ready(root) =
            kenon_core::data_root::check(dir.path())
        else {
            panic!("expected a data root");
        };
        let bridge = Arc::new(Bridge::new(root));
        assert_eq!(
            respond(Some(bridge.clone()), "/nothing", None).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            respond(Some(bridge), "/sheet%2F5%3A9%2F5%3A3%2F0", None).status(),
            StatusCode::NOT_FOUND
        );
    }
}

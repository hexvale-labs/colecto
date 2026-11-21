//! Untrusted HTML email sanitization and safe inline image (`cid:`) resolution.
use base64::Engine;
use chacha20poly1305::aead::{OsRng, rand_core::RngCore};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// An inline MIME image part referenced by `cid:<content-id>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineImage {
    pub content_type: String,
    pub data: Vec<u8>,
}

/// Options controlling HTML sanitization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SanitizeOptions {
    /// Whether to preserve remote `http://` / `https://` / `//` image sources.
    /// Off by default to block tracking pixels.
    pub allow_remote_images: bool,
}

/// Sanitized HTML output ready for sandboxed rendering.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SanitizedHtml {
    pub html: String,
    pub blocked_remote_images: usize,
}

/// Only safe raster formats may be inlined as `data:` URIs (never SVG).
pub(crate) fn is_safe_raster_mime(content_type: &str) -> bool {
    matches!(
        content_type.trim().to_ascii_lowercase().as_str(),
        "image/png"
            | "image/jpeg"
            | "image/jpg"
            | "image/gif"
            | "image/webp"
            | "image/avif"
            | "image/bmp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
    )
}

/// Normalize a `Content-ID` header value or `cid:` URI component.
pub(crate) fn normalize_cid(value: &str) -> String {
    let trimmed = value.trim();
    let stripped = trimmed
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or(trimmed);
    stripped.trim().to_ascii_lowercase()
}

fn has_scheme(value: &str, scheme: &str) -> bool {
    value
        .get(..scheme.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
}

fn is_remote_url(value: &str) -> bool {
    has_scheme(value, "http://") || has_scheme(value, "https://") || value.starts_with("//")
}

/// Sanitize untrusted HTML email markup:
/// - strips scripts, styles, forms, iframes, objects, SVGs, and inline event handlers
/// - drops `javascript:`, `vbscript:`, and untrusted `data:` URLs
/// - blocks remote images unless `options.allow_remote_images` is true, counting blocked ones
/// - rewrites valid `cid:` image references to safe raster `data:` URIs after sanitization
pub fn sanitize(
    raw_html: &str,
    inline_images: &BTreeMap<String, InlineImage>,
    options: SanitizeOptions,
) -> SanitizedHtml {
    let mut nonce_bytes = [0u8; 16];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce: String = nonce_bytes.iter().map(|b| format!("{b:02x}")).collect();

    let blocked_remote_images = Arc::new(AtomicUsize::new(0));
    let blocked_cb = Arc::clone(&blocked_remote_images);
    let replacements: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let replacements_cb = Arc::clone(&replacements);
    let allow_remote = options.allow_remote_images;
    let valid_cids: HashSet<String> = inline_images
        .iter()
        .filter(|(_, img)| is_safe_raster_mime(&img.content_type))
        .map(|(cid, _)| normalize_cid(cid))
        .collect();

    // Keep ammonia's conservative default tag allowlist (excludes form, input, button, textarea, select, meta, base, link).
    let mut builder = ammonia::Builder::default();
    builder.clean_content_tags(HashSet::from([
        "script", "style", "iframe", "object", "embed", "noscript", "svg", "math", "template",
    ]));
    builder.url_schemes(HashSet::from(["http", "https", "mailto", "cid"]));
    builder.url_relative(ammonia::UrlRelative::Deny);
    builder.link_rel(Some("noopener noreferrer"));
    builder.rm_generic_attributes(["srcset", "poster", "background"]);
    builder.attribute_filter(move |element, attribute, value| {
        let trimmed = value.trim();
        if element == "img" && attribute == "src" {
            if has_scheme(trimmed, "cid:") {
                let cid = normalize_cid(&trimmed[4..]);
                if !valid_cids.contains(&cid) {
                    return None;
                }
                let mut guard = replacements_cb.lock().unwrap_or_else(|e| e.into_inner());
                let sentinel = format!("https://cid.colecto.invalid/{nonce}/{}/", guard.len());
                guard.push((sentinel.clone(), cid));
                return Some(sentinel.into());
            }
            if is_remote_url(trimmed) {
                if allow_remote {
                    return Some(trimmed.to_owned().into());
                }
                blocked_cb.fetch_add(1, Ordering::Relaxed);
                return None;
            }
            return None;
        }
        if element == "a" && attribute == "href" {
            if has_scheme(trimmed, "http://")
                || has_scheme(trimmed, "https://")
                || has_scheme(trimmed, "mailto:")
            {
                return Some(trimmed.to_owned().into());
            }
            return None;
        }
        if matches!(attribute, "srcset" | "poster" | "background") {
            return None;
        }
        Some(value.into())
    });

    let mut html = builder.clean(raw_html).to_string();
    drop(builder);
    let pending: Vec<(String, String)> = replacements
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect();
    for (sentinel, cid) in pending {
        let img = inline_images.get(&cid).or_else(|| {
            inline_images
                .iter()
                .find(|(k, _)| normalize_cid(k) == cid)
                .map(|(_, v)| v)
        });
        if let Some(img) = img {
            let mime = img.content_type.trim().to_ascii_lowercase();
            let encoded = base64::engine::general_purpose::STANDARD.encode(&img.data);
            let data_uri = format!("data:{mime};base64,{encoded}");
            html = html.replace(&sentinel, &data_uri);
        }
    }

    SanitizedHtml {
        html,
        blocked_remote_images: blocked_remote_images.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_dangerous_tags_handlers_and_urls() {
        let dirty = r#"
            <script>alert(1)</script>
            <style>body{display:none}</style>
            <iframe src="https://evil.test"></iframe>
            <form action="https://evil.test"><input type="password"><button>Go</button></form>
            <p onclick="alert(1)">Hello</p>
            <img src="x" onerror="alert(1)">
            <a href="javascript:alert(1)">Bad Link</a>
            <a href="https://example.test/ok">Good Link</a>
            <a href="mailto:hi@example.test">Mail</a>
        "#;
        let sanitized = sanitize(dirty, &BTreeMap::new(), SanitizeOptions::default());
        let out = &sanitized.html;
        assert!(!out.contains("<script"));
        assert!(!out.contains("alert(1)"));
        assert!(!out.contains("<style"));
        assert!(!out.contains("<iframe"));
        assert!(!out.contains("<form"));
        assert!(!out.contains("<input"));
        assert!(!out.contains("<button"));
        assert!(!out.contains("onclick"));
        assert!(!out.contains("onerror"));
        assert!(!out.contains("javascript:"));
        assert!(out.contains("<p>Hello</p>"));
        assert!(out.contains(r#"href="https://example.test/ok""#));
        assert!(out.contains(r#"rel="noopener noreferrer""#));
        assert!(out.contains(r#"href="mailto:hi@example.test""#));
    }

    #[test]
    fn strips_untrusted_data_uris_even_when_remote_images_allowed() {
        let dirty = r#"
            <img src="data:image/svg+xml,<svg onload=alert(1)>">
            <img src="data:image/png;base64,AAAA">
            <a href="data:text/html,<script>alert(1)</script>">Click</a>
        "#;
        let sanitized = sanitize(
            dirty,
            &BTreeMap::new(),
            SanitizeOptions {
                allow_remote_images: true,
            },
        );
        assert!(!sanitized.html.contains("data:"));
        assert_eq!(sanitized.blocked_remote_images, 0);
    }

    #[test]
    fn blocks_and_counts_remote_images_by_default_and_allows_when_enabled() {
        let html =
            r#"<p>Look:</p><img src="https://track.test/p.gif"><img src="http://cdn.test/a.png">"#;
        let blocked = sanitize(html, &BTreeMap::new(), SanitizeOptions::default());
        assert_eq!(blocked.blocked_remote_images, 2);
        assert!(!blocked.html.contains("track.test"));
        assert!(!blocked.html.contains("cdn.test"));

        let allowed = sanitize(
            html,
            &BTreeMap::new(),
            SanitizeOptions {
                allow_remote_images: true,
            },
        );
        assert_eq!(allowed.blocked_remote_images, 0);
        assert!(allowed.html.contains(r#"src="https://track.test/p.gif""#));
        assert!(allowed.html.contains(r#"src="http://cdn.test/a.png""#));
    }

    #[test]
    fn resolves_valid_raster_cid_images_and_rejects_svg_or_missing_cids() {
        let mut inline = BTreeMap::new();
        inline.insert(
            "logo@example".into(),
            InlineImage {
                content_type: "image/png".into(),
                data: b"PNGDATA".to_vec(),
            },
        );
        inline.insert(
            "vector@example".into(),
            InlineImage {
                content_type: "image/svg+xml".into(),
                data: b"<svg/>".to_vec(),
            },
        );
        let html = r#"
            <img src="CID:Logo@Example" alt="Logo">
            <img src="cid:vector@example" alt="SVG">
            <img src="cid:missing@example" alt="Missing">
            <a href="cid:logo@example">Link</a>
        "#;
        let sanitized = sanitize(html, &inline, SanitizeOptions::default());
        assert!(
            sanitized
                .html
                .contains(r#"src="data:image/png;base64,UE5HREFUQQ==""#)
        );
        assert!(!sanitized.html.contains("svg+xml"));
        assert!(!sanitized.html.contains("missing@example"));
        assert!(!sanitized.html.contains(r#"href="cid:"#));
        assert!(!sanitized.html.contains("cid.colecto.invalid"));
    }

    #[test]
    fn resolves_twelve_or_more_inline_cid_images_without_prefix_collision() {
        let mut inline = BTreeMap::new();
        let mut html = String::new();
        for i in 0..12 {
            let cid = format!("img{i}@example");
            inline.insert(
                cid.clone(),
                InlineImage {
                    content_type: "image/png".into(),
                    data: format!("IMG-{i:02}").into_bytes(),
                },
            );
            html.push_str(&format!(r#"<img src="cid:{cid}">"#));
        }
        let sanitized = sanitize(&html, &inline, SanitizeOptions::default());
        assert!(!sanitized.html.contains("cid.colecto.invalid"));
        for i in 0..12 {
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("IMG-{i:02}").as_bytes());
            let expected = format!(r#"src="data:image/png;base64,{encoded}""#);
            assert!(
                sanitized.html.contains(&expected),
                "missing expected data URI for image {i} in {}",
                sanitized.html
            );
        }
    }
}

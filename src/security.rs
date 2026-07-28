//! Input sanitization and validation.
//!
//! Every value that reaches the database or is echoed back to a browser passes
//! through this module. The goals, in order:
//!
//! 1. **No stored XSS** – document content is HTML written by a `contenteditable`
//!    editor, so it cannot be escaped wholesale. It is instead re-parsed and
//!    re-serialized by `ammonia` against a strict tag/attribute allow-list, which
//!    drops `<script>`, event handlers, `javascript:` URLs and unknown elements.
//! 2. **No NoSQL operator injection** – identifiers are validated as UUIDs and
//!    user ids are restricted to a conservative character class, so no value can
//!    ever be interpreted as a BSON operator document.
//! 3. **No command / path injection** – the process never shells out and never
//!    builds a filesystem path from user input; uploads are read into memory
//!    only. The helpers here additionally strip control characters so nothing
//!    that reaches a log line can inject terminal escape sequences.

use std::collections::{HashMap, HashSet};

use once_cell::sync::Lazy;

use crate::compile_config::{
    DEFAULT_TITLE, MAX_CONTENT_BYTES, MAX_TITLE_CHARS, MAX_USER_ID_CHARS,
    SANITIZER_ALLOWED_ATTRIBUTES, SANITIZER_ALLOWED_CLASSES, SANITIZER_ALLOWED_IFRAME_HOSTS,
    SANITIZER_ALLOWED_TAGS, SANITIZER_ALLOWED_URL_SCHEMES,
};

/// A rejected input, reported to the client as a 400 with this message.
#[derive(Debug, PartialEq, Eq)]
pub struct ValidationError(pub String);

impl ValidationError {
    fn new(message: impl Into<String>) -> Self {
        ValidationError(message.into())
    }
}

static SANITIZER: Lazy<ammonia::Builder<'static>> = Lazy::new(build_sanitizer);

fn build_sanitizer() -> ammonia::Builder<'static> {
    let mut builder = ammonia::Builder::default();

    let tags: HashSet<&str> = SANITIZER_ALLOWED_TAGS.iter().copied().collect();
    let mut attributes: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (tag, attrs) in SANITIZER_ALLOWED_ATTRIBUTES.iter() {
        attributes.insert(tag, attrs.iter().copied().collect());
    }
    let schemes: HashSet<&str> = SANITIZER_ALLOWED_URL_SCHEMES.iter().copied().collect();
    let mut classes: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (tag, names) in SANITIZER_ALLOWED_CLASSES.iter() {
        classes.insert(tag, names.iter().copied().collect());
    }

    builder
        .tags(tags)
        .tag_attributes(attributes)
        .url_schemes(schemes)
        .generic_attributes(HashSet::new())
        .allowed_classes(classes)
        .url_relative(ammonia::UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .strip_comments(true)
        .attribute_filter(|element, attribute, value| {
            // Only YouTube embeds may appear in an iframe `src`.
            if element == "iframe" && attribute == "src" {
                return if is_allowed_embed_url(value) {
                    Some(value.to_string().into())
                } else {
                    None
                };
            }
            Some(value.to_string().into())
        });

    builder
}

/// True when `value` is an `https` URL on an allow-listed embed host.
fn is_allowed_embed_url(value: &str) -> bool {
    let rest = match value.strip_prefix("https://") {
        Some(rest) => rest,
        None => return false,
    };

    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    // Reject embedded credentials (`user:pass@host`) used to spoof the host.
    if host.contains('@') || host.is_empty() {
        return false;
    }

    SANITIZER_ALLOWED_IFRAME_HOSTS.contains(&host.as_str())
}

/// Sanitize editor HTML, rejecting content that is too large to store.
pub fn sanitize_content(raw: &str) -> Result<String, ValidationError> {
    if raw.len() > MAX_CONTENT_BYTES {
        return Err(ValidationError::new(
            "Document content is too large (limit 512 KB)",
        ));
    }

    // An iframe whose `src` was rejected is dropped entirely rather than left as
    // an empty frame in the document.
    let clean = SANITIZER.clean(raw).to_string().replace("<iframe></iframe>", "");

    if clean.len() > MAX_CONTENT_BYTES {
        return Err(ValidationError::new(
            "Document content is too large (limit 512 KB)",
        ));
    }

    Ok(clean)
}

/// Normalize a document title: strip control characters, collapse whitespace,
/// enforce a length limit and fall back to a default when blank.
pub fn sanitize_title(raw: &str) -> Result<String, ValidationError> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    if cleaned.chars().count() > MAX_TITLE_CHARS {
        return Err(ValidationError::new("Title is too long (limit 200 characters)"));
    }

    if cleaned.is_empty() {
        return Ok(DEFAULT_TITLE.to_string());
    }

    // Titles are rendered as text nodes only; escaping happens in the client via
    // `textContent`, and any HTML-looking value is stored inert.
    Ok(cleaned)
}

/// Validate a user identifier. Mocked auth still means untrusted input, so the
/// character class is deliberately narrow: it cannot express a BSON operator,
/// a path segment or a shell metacharacter.
pub fn validate_user_id(raw: &str) -> Result<String, ValidationError> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Err(ValidationError::new("A user id is required"));
    }

    if trimmed.chars().count() > MAX_USER_ID_CHARS {
        return Err(ValidationError::new("User id is too long (limit 64 characters)"));
    }

    let valid = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'));

    if !valid {
        return Err(ValidationError::new(
            "User id may only contain letters, digits, '.', '_', '-' and '@'",
        ));
    }

    Ok(trimmed.to_ascii_lowercase())
}

/// Validate a document identifier as a UUID so it can never carry a query
/// operator or a path traversal sequence into the data layer.
pub fn validate_document_id(raw: &str) -> Result<String, ValidationError> {
    uuid::Uuid::parse_str(raw.trim())
        .map(|id| id.to_string())
        .map_err(|_| ValidationError::new("Invalid document id"))
}

/// Validate a share permission value.
pub fn validate_permission(raw: &str) -> Result<String, ValidationError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "view" => Ok("view".to_string()),
        "edit" => Ok("edit".to_string()),
        _ => Err(ValidationError::new("Permission must be 'view' or 'edit'")),
    }
}

/// Strip control characters from a value before it is written to a log line.
pub fn safe_for_log(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_and_event_handlers() {
        let dirty = r#"<p onclick="alert(1)">hi</p><script>fetch('/x')</script><img src=x onerror=alert(1)>"#;
        let clean = sanitize_content(dirty).unwrap();
        assert_eq!(clean, "<p>hi</p>");
    }

    #[test]
    fn strips_javascript_urls_and_unknown_tags() {
        let clean =
            sanitize_content(r#"<a href="javascript:alert(1)">x</a><object data="y"></object>"#)
                .unwrap();
        assert!(!clean.contains("javascript"));
        assert!(!clean.contains("object"));
    }

    #[test]
    fn keeps_formatting_and_youtube_embeds() {
        let dirty = concat!(
            "<h1>T</h1><p><b>b</b><i>i</i><u>u</u></p><ul><li>x</li></ul>",
            r#"<iframe src="https://www.youtube-nocookie.com/embed/abc123"></iframe>"#
        );
        let clean = sanitize_content(dirty).unwrap();
        assert!(clean.contains("<h1>T</h1>"));
        assert!(clean.contains("<li>x</li>"));
        assert!(clean.contains("youtube-nocookie.com/embed/abc123"));
    }

    #[test]
    fn keeps_embed_wrapper_class() {
        let clean = sanitize_content(
            r#"<div class="embed evil"><iframe src="https://www.youtube.com/embed/aaaaaaaaaaa"></iframe></div>"#,
        )
        .unwrap();
        assert!(clean.contains(r#"<div class="embed">"#));
    }

    #[test]
    fn drops_non_youtube_iframes() {
        let clean =
            sanitize_content(r#"<iframe src="https://evil.example.com/x"></iframe>"#).unwrap();
        assert!(!clean.contains("evil.example.com"));
        assert!(!clean.contains("iframe"));

        let spoofed = sanitize_content(
            r#"<iframe src="https://www.youtube.com@evil.example.com/x"></iframe>"#,
        )
        .unwrap();
        assert!(!spoofed.contains("evil.example.com"));
    }

    #[test]
    fn rejects_oversized_content() {
        let big = "a".repeat(MAX_CONTENT_BYTES + 1);
        assert!(sanitize_content(&big).is_err());
    }

    #[test]
    fn user_ids_reject_injection_payloads() {
        assert!(validate_user_id(r#"{"$ne": null}"#).is_err());
        assert!(validate_user_id("user1; rm -rf /").is_err());
        assert!(validate_user_id("../../etc/passwd").is_err());
        assert!(validate_user_id("$where").is_err());
        assert_eq!(validate_user_id("  Alice@Example.com ").unwrap(), "alice@example.com");
    }

    #[test]
    fn document_ids_must_be_uuids() {
        assert!(validate_document_id("../../etc/passwd").is_err());
        assert!(validate_document_id(r#"{"$gt":""}"#).is_err());
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(validate_document_id(&id).unwrap(), id);
    }

    #[test]
    fn titles_are_normalized_and_bounded() {
        assert_eq!(sanitize_title("  spaced\u{0007}  out  ").unwrap(), "spaced out");
        assert_eq!(sanitize_title("   ").unwrap(), DEFAULT_TITLE);
        assert!(sanitize_title(&"t".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }
}

/// Policy tests.
///
/// These are deliberately written against the *configuration* in
/// `compile_config`, not only against the sanitizer's current behaviour. The
/// sanitizer is only as good as its allow-list, so widening that list - adding
/// `style`, `srcdoc`, `target`, a `data:` scheme or a second embed provider -
/// must fail the build rather than quietly reopening an XSS hole.
#[cfg(test)]
mod policy_tests {
    use super::*;
    use crate::compile_config::{
        HEADER_CSP, SANITIZER_ALLOWED_ATTRIBUTES, SANITIZER_ALLOWED_CLASSES,
        SANITIZER_ALLOWED_IFRAME_HOSTS, SANITIZER_ALLOWED_TAGS, SANITIZER_ALLOWED_URL_SCHEMES,
    };

    /// Constructs that must never appear in stored content, whatever the input.
    const FORBIDDEN_IN_OUTPUT: [&str; 21] = [
        "<script",
        "</script",
        "<style",
        "<object",
        "<embed",
        "<form",
        "<base",
        "<link",
        "<meta",
        "<svg",
        "<math",
        "<applet",
        "<template",
        "javascript:",
        "vbscript:",
        "data:text/html",
        "srcdoc",
        "formaction",
        "onerror",
        "onload",
        "style=",
    ];

    /// Injection corpus. Every entry is sanitized and checked against
    /// `FORBIDDEN_IN_OUTPUT`; several also assert a specific expected shape.
    const CORPUS: [&str; 34] = [
        r#"<script>alert(1)</script>"#,
        r#"<SCRIPT SRC=//evil.test/x.js></SCRIPT>"#,
        r#"<scr<script>ipt>alert(1)</script>"#,
        r#"<p onclick="alert(1)">x</p>"#,
        r#"<p onmouseover=alert(1)>x</p>"#,
        r#"<div onfocus="alert(1)" autofocus>x</div>"#,
        r#"<img src=x onerror=alert(1)>"#,
        r#"<img src="javascript:alert(1)">"#,
        r#"<a href="javascript:alert(1)">click</a>"#,
        r#"<a href="JaVaScRiPt:alert(1)">click</a>"#,
        r#"<a href="&#106;avascript:alert(1)">click</a>"#,
        r#"<a href="data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==">x</a>"#,
        r#"<iframe src="https://evil.test/frame"></iframe>"#,
        r#"<iframe src="//evil.test/frame"></iframe>"#,
        r#"<iframe src="https://www.youtube-nocookie.com.evil.test/embed/x"></iframe>"#,
        r#"<iframe src="https://evil.test/#www.youtube.com"></iframe>"#,
        r#"<iframe srcdoc="<script>alert(1)</script>"></iframe>"#,
        r#"<iframe src="javascript:alert(1)"></iframe>"#,
        r#"<svg><script>alert(1)</script></svg>"#,
        r#"<svg onload=alert(1)></svg>"#,
        r#"<math><mtext><script>alert(1)</script></mtext></math>"#,
        r#"<style>body{background:url('javascript:alert(1)')}</style>"#,
        r#"<p style="background:url(javascript:alert(1))">x</p>"#,
        r#"<div style="position:fixed;top:0;left:0;width:100vw;height:100vw">clickjack</div>"#,
        r#"<object data="data:text/html,<script>alert(1)</script>"></object>"#,
        r#"<embed src="https://evil.test/x.swf">"#,
        r#"<form action="https://evil.test"><button formaction="https://evil.test">go</button></form>"#,
        r#"<base href="https://evil.test/">"#,
        r#"<link rel="stylesheet" href="https://evil.test/x.css">"#,
        r#"<meta http-equiv="refresh" content="0;url=https://evil.test">"#,
        r#"<body onload=alert(1)>x</body>"#,
        r#"<template><script>alert(1)</script></template>"#,
        r#"<p>legit <b>bold</b> and <i>italic</i></p><ul><li>one</li></ul>"#,
        r#"<div class="embed"><iframe src="https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"></iframe></div>"#,
    ];

    #[test]
    fn injection_corpus_produces_no_dangerous_output() {
        for payload in CORPUS {
            let clean = sanitize_content(payload).expect("sanitizer must not error");
            let lowered = clean.to_ascii_lowercase();
            for forbidden in FORBIDDEN_IN_OUTPUT {
                assert!(
                    !lowered.contains(forbidden),
                    "payload {:?} produced {:?} which contains {:?}",
                    payload,
                    clean,
                    forbidden
                );
            }
        }
    }

    #[test]
    fn only_allow_listed_iframe_hosts_survive() {
        for payload in CORPUS {
            let clean = sanitize_content(payload).unwrap();
            for fragment in clean.split("<iframe").skip(1) {
                let src_start = fragment.find("src=\"").expect("iframe kept without src");
                let rest = &fragment[src_start + 5..];
                let src = &rest[..rest.find('"').unwrap()];
                let host_ok = SANITIZER_ALLOWED_IFRAME_HOSTS
                    .iter()
                    .any(|host| src.starts_with(&format!("https://{}/", host)));
                assert!(host_ok, "iframe survived with src {:?}", src);
            }
        }
    }

    #[test]
    fn legitimate_formatting_and_youtube_embed_survive() {
        let clean = sanitize_content(CORPUS[32]).unwrap();
        assert!(clean.contains("<b>bold</b>") || clean.contains("<strong>bold</strong>"));
        assert!(clean.contains("<li>one</li>"));

        let clean = sanitize_content(CORPUS[33]).unwrap();
        assert!(clean.contains("class=\"embed\""));
        assert!(clean.contains("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"));
    }

    #[test]
    fn allow_list_stays_narrow() {
        for tag in SANITIZER_ALLOWED_TAGS {
            assert!(
                !matches!(
                    tag,
                    "script"
                        | "style"
                        | "object"
                        | "embed"
                        | "form"
                        | "input"
                        | "button"
                        | "base"
                        | "link"
                        | "meta"
                        | "svg"
                        | "math"
                        | "applet"
                        | "template"
                        | "body"
                        | "a"
                ),
                "tag {:?} must not be allow-listed",
                tag
            );
        }

        for (tag, attributes) in SANITIZER_ALLOWED_ATTRIBUTES {
            for attribute in attributes {
                assert!(
                    !attribute.starts_with("on"),
                    "{}: event handler attribute {:?} must not be allow-listed",
                    tag,
                    attribute
                );
                assert!(
                    !matches!(*attribute, "style" | "srcdoc" | "formaction" | "target" | "href"),
                    "{}: attribute {:?} must not be allow-listed",
                    tag,
                    attribute
                );
            }
        }

        assert_eq!(
            SANITIZER_ALLOWED_URL_SCHEMES,
            ["https"],
            "only https URLs may be stored: http, data and javascript stay out"
        );

        assert_eq!(
            SANITIZER_ALLOWED_CLASSES.len(),
            1,
            "the class allow-list exists only for the responsive embed wrapper"
        );

        for host in SANITIZER_ALLOWED_IFRAME_HOSTS {
            assert!(
                host.ends_with("youtube.com") || host.ends_with("youtube-nocookie.com"),
                "iframe host {:?} is not a YouTube embed host",
                host
            );
            assert!(
                !host.contains('*') && !host.contains('/'),
                "iframe host {:?} must be a bare host name",
                host
            );
        }
    }

    /// The sanitizer is defence in depth *with* the CSP, so the CSP must stay
    /// strict and stay in sync with the iframe allow-list.
    #[test]
    fn csp_stays_strict_and_matches_the_iframe_allow_list() {
        let csp = HEADER_CSP.1;
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("base-uri 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("form-action 'none'"));
        assert!(csp.contains("script-src 'self'"));
        assert!(!csp.contains("unsafe-inline"), "CSP must not allow inline code");
        assert!(!csp.contains("unsafe-eval"), "CSP must not allow eval");
        assert!(!csp.contains(" *"), "CSP must not use a wildcard source");
        assert!(!csp.contains("http://"), "CSP must not allow plaintext sources");

        for host in SANITIZER_ALLOWED_IFRAME_HOSTS {
            assert!(
                csp.contains(&format!("https://{}", host)),
                "frame-src is missing the allow-listed embed host {:?}",
                host
            );
        }
    }
}

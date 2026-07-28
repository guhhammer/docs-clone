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
